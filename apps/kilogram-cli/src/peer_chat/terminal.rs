use super::*;
use std::collections::HashMap;
use tokio::sync::{Mutex as AsyncMutex, oneshot};

struct Bridge {
    input: Arc<Mutex<io::PipeWriter>>,
    serial: AsyncMutex<()>,
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<Value>>>>,
}
async fn confirm_pending(local: &Arc<Mutex<Local>>, ui: &Ui) -> Result<()> {
    let candidate = local.try_lock().ok().and_then(|l| l.approval.clone());
    if let Some((peer, artifact)) = candidate {
        let prompt = ui.clone();
        let account = artifact.sender.account.clone();
        tokio::task::spawn_blocking(move || prompt.confirm(&format!("Собеседник: {account}. Сверьте identity по доверенному каналу. Разрешить переписку?"),"/accept")).await??;
        local
            .lock()
            .map_err(|_| anyhow::anyhow!("Account unavailable"))?
            .approve(peer, &artifact)?;
    }
    Ok(())
}
impl Bridge {
    async fn call(&self, op: &str, args: Value) -> Result<Value> {
        let _serial = if op == "quit" {
            None
        } else {
            Some(self.serial.lock().await)
        };
        let retry_until = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            let response = self.frame(op, &args).await?;
            if op == "send"
                && response["ok"] == false
                && response["code"] == "pending_request"
                && std::time::Instant::now() < retry_until
            {
                // This refusal occurs BEFORE this new request is protected or
                // dispatched. Reuse its immutable business request_id/text;
                // never retry an uncertain/admitted send or change its ID.
                tokio::time::sleep(Duration::from_millis(20)).await;
                continue;
            }
            ensure!(response["ok"] == true, "Chat {}: {}", op, response["code"]);
            return Ok(response["value"].clone());
        }
    }
    async fn frame(&self, op: &str, args: &Value) -> Result<Value> {
        let id = artifact::id()?;
        let (tx, rx) = oneshot::channel();
        self.pending
            .lock()
            .map_err(|_| anyhow::anyhow!("Chat replies unavailable"))?
            .insert(id.clone(), tx);
        let raw = serde_json::to_vec(&json!({"v":1,"id":id,"op":op,"args":args}))?;
        let input = self.input.clone();
        let sent = tokio::task::spawn_blocking(move || -> Result<()> {
            let mut input = input
                .lock()
                .map_err(|_| anyhow::anyhow!("Chat input unavailable"))?;
            input.write_all(&raw)?;
            input.write_all(b"\n")?;
            input.flush()?;
            Ok(())
        })
        .await?;
        if let Err(error) = sent {
            self.pending.lock().ok().map(|mut p| p.remove(&id));
            return Err(error);
        }
        let result = timeout(Duration::from_secs(45), rx).await;
        self.pending.lock().ok().map(|mut p| p.remove(&id));
        let response =
            result.context("Chat operation deadline; saved pending work is preserved")??;
        Ok(response)
    }
}
struct Owner {
    thread: std::thread::JoinHandle<()>,
    done: oneshot::Receiver<i32>,
}
async fn start(setup: Setup, ui: &Ui) -> Result<(Owner, Arc<Bridge>)> {
    let (commands, input) = io::pipe()?;
    let (stdout, output) = io::pipe()?;
    let (done_tx, done) = oneshot::channel();
    let thread = std::thread::Builder::new()
        .name("peer-chat-owner".into())
        .spawn(move || {
            let code = crate::terminal_chat::run_product_owned(
                setup,
                true,
                io::BufReader::new(commands),
                output,
            );
            let _ = done_tx.send(code);
        })?;
    let pending = Arc::new(Mutex::new(HashMap::<String, oneshot::Sender<Value>>::new()));
    let (ready_tx, ready_rx) = oneshot::channel();
    let replies = pending.clone();
    let output = ui.clone();
    std::thread::spawn(move || {
        let mut ready_tx = Some(ready_tx);
        let lines = io::BufReader::new(stdout).lines();
        for line in lines {
            let Ok(line) = line else { break };
            if line.len() > 256 * 1024 {
                break;
            }
            let Ok(value) = serde_json::from_str::<Value>(&line) else {
                break;
            };
            if value["type"] == "ready"
                && let Some(tx) = ready_tx.take()
            {
                let _ = tx.send(());
            }
            if let Some(id) = value["id"].as_str() {
                if let Some(tx) = replies.lock().ok().and_then(|mut p| p.remove(id)) {
                    let _ = tx.send(value);
                }
            } else if value["event"] == "incoming" {
                output.emit(
                    "message",
                    &format!("Собеседник: {}", value["text"].as_str().unwrap_or_default()),
                    value,
                );
            }
        }
        if let Ok(mut p) = replies.lock() {
            p.clear();
        }
    });
    let bridge = Arc::new(Bridge {
        input: Arc::new(Mutex::new(input)),
        serial: AsyncMutex::new(()),
        pending,
    });
    timeout(Duration::from_secs(10), ready_rx)
        .await
        .context("Chat readiness deadline")??;
    Ok((Owner { thread, done }, bridge))
}

pub(super) async fn run(home: PathBuf, local: &Arc<Mutex<Local>>, ui: &Ui) -> Result<()> {
    let ctrl_c = tokio::signal::ctrl_c();
    tokio::pin!(ctrl_c);
    loop {
        if ui.stop.load(Ordering::Acquire) {
            return Ok(());
        }
        confirm_pending(local, ui).await?;
        let ready = local.try_lock().is_ok_and(|l| l.ready);
        if ready {
            break;
        }
        if let Ok(input) = ui.input.try_lock() {
            match input.try_recv() {
                Ok(line) if line == "/quit" => return Ok(()),
                Ok(_) => ui.emit(
                    "notice",
                    "Ожидаю принятие приглашения. /quit — выход.",
                    Value::Null,
                ),
                Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        tokio::select! {_=tokio::time::sleep(Duration::from_millis(50))=>{},_=&mut ctrl_c=>return Ok(())}
    }
    // Move the same Setup/file lock into the regular owner; never unlock/reopen.
    let (own_role, setup) = loop {
        if ui.stop.load(Ordering::Acquire) {
            return Ok(());
        }
        let mut l = match local.try_lock() {
            Ok(local) => local,
            Err(std::sync::TryLockError::WouldBlock) => {
                tokio::time::sleep(Duration::from_millis(20)).await;
                continue;
            }
            Err(_) => bail!("Account unavailable"),
        };
        let role = consent::role(l.setup.as_ref().context("Account setup unavailable")?)?;
        l.chat_started = true;
        let setup = l.setup.take().context("Account ownership unavailable")?;
        break (role, setup);
    };
    let (mut owner, bridge) = start(setup, ui).await?;
    let connected = bridge.call("connect", json!({"mode":"start"})).await;
    let replay = Arc::new(AtomicBool::new(false));
    let presence: reader::Presence = Arc::new(Mutex::new(None));
    let reader = connected
        .as_ref()
        .ok()
        .and_then(|v| v["runtime_instance"].as_str())
        .map(|instance| {
            let home = home.clone();
            let instance = instance.to_owned();
            let ui = ui.clone();
            let replay = replay.clone();
            let presence = presence.clone();
            tokio::task::spawn_blocking(move || reader::run(home, instance, ui, replay, presence))
        });
    let mut network_task = None;
    let result=async {
        let connected=connected?;
        let contexts:Vec<String>=serde_json::from_value(connected["contexts"].clone())?;
        ensure!(contexts.len()==2,"Runtime contexts");
        ui.emit("ready","Переписка открыта. Пишите сообщение; /status /history /quit. /resync — повторное отображение истории.",connected);
        network_task=Some(tokio::spawn({let local=local.clone();let bridge=bridge.clone();let ui=ui.clone();let home=home.clone();let contexts=contexts.clone();async move {
            let mut fingerprint=String::new();let mut imported=String::new();let mut publication=None;
            while !ui.stop.load(Ordering::Acquire){
                // Only changed signed locator bytes cause a new export. No per-tick files.
                let own:Result<Vec<_>>=contexts.iter().map(|c|model::read(&home.join("connection/locators").join(format!("{c}.locator")),8192)).collect();
                if let Ok(own)=own {
                    let new=hash(&own.concat());
                    if new!=fingerprint && let Ok(report)=bridge.call("locator_export",json!({})).await
                        && report["state"]=="exported"
                        && let Some(path)=report["path"].as_str()
                        && let Ok(bytes)=model::read(Path::new(path),48*1024)
                    {
                        publication=Some(URL_SAFE_NO_PAD.encode(bytes));fingerprint=new;
                        // This exact fresh export was just read into bounded memory.
                        // The signed runtime locators remain authoritative on disk.
                        if let Err(error)=std::fs::remove_file(path){ui.emit("diagnostic",&format!("Locator export cleanup unconfirmed: {error}"),Value::Null);ui.stop.store(true,Ordering::Release);bail!("Locator export cleanup unconfirmed");}
                    }
                }
                if let Some(encoded)=&publication && let Ok(mut l)=local.try_lock(){l.public.locators=Some(encoded.clone());publication=None;}
                let incoming=local.try_lock().ok().and_then(|mut l|l.incoming.take());
                if let Some(encoded)=incoming && let Ok(raw)=URL_SAFE_NO_PAD.decode(encoded) && raw.len()<=48*1024 {
                    let digest=hash(&raw);
                    if digest!=imported {
                        if let Ok(mut staged)=tempfile::Builder::new().prefix(".peer-incoming-").suffix(".json").tempfile_in(home.join("connection")) {
                            if staged.write_all(&raw).and_then(|()|staged.as_file().sync_all()).is_ok() {
                                match bridge.call("locator_import_latest",json!({"path":staged.path()})).await {
                                    Ok(report) if report["state"]=="imported"=>imported=digest,
                                    Err(error) if error.to_string().ends_with("retained_locator_cleanup_unconfirmed")=>{ui.emit("diagnostic",&error.to_string(),Value::Null);ui.stop.store(true,Ordering::Release);return Err(error);}
                                    _=>{}
                                }
                            }
                            // The actual importer owns its validated retained copy.
                            // Rejected/unconfirmed staging is removed by RAII too.
                            if let Err(error)=staged.close(){ui.emit("diagnostic",&format!("Locator staging cleanup unconfirmed: {error}"),Value::Null);ui.stop.store(true,Ordering::Release);bail!("Locator staging cleanup unconfirmed");}
                        }
                    }
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Ok::<_,anyhow::Error>(())
        }}));
        let mut slot=0usize;
        let mut history_cursor:[Option<String>;2]=[None,None];
        loop {
            if ui.stop.load(Ordering::Acquire){break}
            confirm_pending(local,ui).await?;
            if owner.thread.is_finished(){bail!("Owned chat exited before shutdown")}
            let input=ui.input.try_lock().ok().map(|input|input.try_recv());
            let line=match input {
                Some(Ok(line))=>line,
                Some(Err(mpsc::TryRecvError::Disconnected))=>break,
                _=>{tokio::select!{_=tokio::time::sleep(Duration::from_millis(10))=>continue,_=&mut ctrl_c=>break}},
            };
            if line=="/quit"{break}
            if line=="/resync"{replay.store(true,Ordering::Release);ui.emit("notice","Повторно читаю сохранённую историю; уже показанные сообщения могут повториться.",Value::Null);continue}
            if line=="/status" {
                let current=presence.lock().ok().and_then(|p|p.as_ref().filter(|(at,_)|at.elapsed()<Duration::from_secs(2)).map(|(_,v)|v.clone()));
                if let Some(current)=current{ui.emit("result",connection_notice(&current),current)}else{ui.emit("notice","Текущее состояние уточняется; сохранённые сообщения остаются локально.",Value::Null)}
                continue;
            }
            if line=="/use 1"||line=="/use 2" {slot=usize::from(line.ends_with('2'));ui.emit("notice",&format!("Выбран чат {}.",slot+1),Value::Null);continue}
            let reply=if line=="/history" || line=="/history more" {
                if line=="/history more" && history_cursor[slot].is_none(){ui.emit("notice","Продолжения истории нет. /history — прочитать заново.",Value::Null);continue}
                bridge.call("history",json!({"context":contexts[slot],"limit":16,"cursor":if line=="/history more"{history_cursor[slot].clone()}else{None}})).await
            }
            else if line.starts_with('/') && !line.starts_with("/send ") {ui.emit("help","Пишите текст; /status /history /resync /quit.",Value::Null);continue}
            else {
                let text=line.strip_prefix("/send ").unwrap_or(&line);
                if !kilogram_first_dialog::send_intent::valid_text(text){ui.emit("error","Пустое или слишком длинное сообщение.",Value::Null);continue}
                let request=&artifact::id()?[..32];
                bridge.call("send",json!({"context":contexts[slot],"request_id":request,"text":text})).await
            };
            match reply {
                Ok(value) if value["items"].is_array()=>{
                    history_cursor[slot]=value["cursor"].as_str().map(str::to_owned);
                    for row in value["items"].as_array().into_iter().flatten(){
                        let speaker=if row["role"]==own_role {"Вы"}else{"Собеседник"};
                        ui.emit("history",&format!("{speaker}: {}",row["text"].as_str().unwrap_or_default()),row.clone());
                    }
                    ui.emit("history_page",if history_cursor[slot].is_some(){"/history more — следующие сообщения."}else{"История прочитана."},json!({"has_more":history_cursor[slot].is_some()}));
                }
                Ok(value)=>ui.emit("result",send_notice(&value),value),
                Err(error)=>ui.emit("operation_error",&format!("{error}. Сохранённые сообщения не удалены."),Value::Null),
            }
        }
        ui.stop.store(true,Ordering::Release);
        Ok::<_,anyhow::Error>(())
    }.await;
    ui.stop.store(true, Ordering::Release);
    let observed = if let Some(reader) = reader {
        match timeout(Duration::from_secs(8), reader).await {
            Ok(Ok(result)) => result,
            Ok(Err(error)) => Err(error.into()),
            Err(error) => Err(error.into()),
        }
    } else {
        Ok(())
    };
    let closed = bridge.call("quit", json!({})).await;
    let network_closed = if let Some(mut network) = network_task {
        match timeout(Duration::from_secs(8), &mut network).await {
            Ok(Ok(result)) => result,
            Ok(Err(error)) => Err(error.into()),
            Err(_) => {
                network.abort();
                let _ = network.await;
                Err(anyhow::anyhow!("Locator task shutdown unconfirmed"))
            }
        }
    } else {
        Ok(())
    };
    let exit = timeout(Duration::from_secs(35), &mut owner.done).await;
    match exit {
        Ok(Ok(0)) if closed.is_ok() => {
            owner
                .thread
                .join()
                .map_err(|_| anyhow::anyhow!("Chat owner thread failed"))?;
        }
        _ => {
            bail!("Chat cleanup unconfirmed; preserve the profile and diagnostics")
        }
    }
    // An ordinary cancelled confirmation is a clean user exit only when all
    // runtime/reader/locator cleanup gates also succeeded.
    user_result(result).and(observed).and(network_closed)
}

fn send_notice(value: &Value) -> &'static str {
    match value["state"].as_str() {
        Some("delivered") => "Доставлено.",
        Some("queued" | "materialized") => {
            "Сохранено локально. Доставка выполняется автоматически."
        }
        _ => "Подтверждение сохранения пока не получено. Повторно сообщение не отправлялось.",
    }
}
pub(super) fn connection_notice(value: &Value) -> &'static str {
    if value["status"]["live_admitted"] != true {
        return "Собеседник offline или переподключается. Сообщения сохраняются локально.";
    }
    match value["status"]["selected_routes"].as_array() {
        Some(routes) if routes.len() == 2 && routes.iter().all(|r| r == "direct") => {
            "Соединение установлено напрямую."
        }
        Some(routes) if routes.iter().any(|r| r == "relay") => {
            "Соединение установлено через relay. Сквозное шифрование сохранено."
        }
        _ => "Соединение подтверждено; текущий путь ещё проверяется.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn usable_peer_ui_does_not_claim_an_uncertain_send_was_saved() {
        for state in ["unknown", "unconfirmed", "retry_available"] {
            assert_eq!(
                send_notice(&json!({"state":state})),
                "Подтверждение сохранения пока не получено. Повторно сообщение не отправлялось."
            );
        }
        assert_eq!(send_notice(&json!({"state":"delivered"})), "Доставлено.");
    }
}
