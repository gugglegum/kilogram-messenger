//! Compile against the isolated exact7f82036 library, never against current S4.
use kilogram_state::{StateDirectoryLock,indexed_dialog::{ExpectedOwner,Config,IndexedDialogVault,CreationClaim,Mutation,Key,Namespace,HistoryOperation,HistoryInit,HistoryScope,Error}};
use std::{error::Error as StdError,path::PathBuf};
use zeroize::Zeroizing;
fn id(text:&str)->Result<[u8;32],Box<dyn StdError>>{if text.len()!=64||!text.is_ascii(){return Err("owner ID width".into());}let mut out=[0;32];for(i,b)in out.iter_mut().enumerate(){*b=u8::from_str_radix(&text[i*2..i*2+2],16)?;}Ok(out)}
fn main()->Result<(),Box<dyn StdError>>{
    let args=std::env::args().skip(1).collect::<Vec<_>>();if args.len()!=7{return Err("mode path account claim logical0 logical1 physical".into());}
    let path=PathBuf::from(&args[1]);let owner=ExpectedOwner{account:id(&args[2])?,claim:id(&args[3])?};
    let config=Config{logical_quota:[args[4].parse()?,args[5].parse()?],physical_quota:args[6].parse()?,retained_external_bytes:0};
    if args[0]=="positive"{
        let mut store=IndexedDialogVault::create(StateDirectoryLock::acquire(&path)?,CreationClaim{account:owner.account,claim:owner.claim,signing_secret:Zeroizing::new([3;32]),encryption_secret:Zeroizing::new([4;32]),contexts:[[5;32],[6;32]]},config)?;
        store.commit(store.generation(),[11;32],vec![Mutation::Put{key:Key::hash(0,Namespace::Control,[12;32])?,value:b"legitimate component".to_vec()}],vec![])?;
        let generation=store.generation();drop(store);
        let mut reopened=IndexedDialogVault::open(StateDirectoryLock::acquire(&path)?,owner,config)?;
        if reopened.generation()!=generation||reopened.get(Key::hash(0,Namespace::Control,[12;32])?)?.as_deref()!=Some(b"legitimate component".as_slice()){return Err("positive component reopen mismatch".into());}
        drop(reopened);
        let history_path=path.with_extension("history");
        let mut history=IndexedDialogVault::create(StateDirectoryLock::acquire(&history_path)?,CreationClaim{account:owner.account,claim:owner.claim,signing_secret:Zeroizing::new([3;32]),encryption_secret:Zeroizing::new([4;32]),contexts:[[5;32],[6;32]]},config)?;
        history.commit_history(0,history.generation(),[13;32],HistoryOperation::Init(HistoryInit{scope:HistoryScope{context:[5;32],dialog:[14;32],epoch:[15;32],session_id:"A".repeat(43)},own_role:0}))?;
        history.commit_history(0,history.generation(),[16;32],HistoryOperation::QueueAccept{request:[17;16],text:"predecessor positive queue".into()})?;
        println!("S3_PREDECESSOR_POSITIVE component_root5=true correct_owner=true separate_generic_and_history_queue_controls=true generation={generation}");return Ok(());
    }
    if args[0]!="probe"{return Err("unknown probe mode".into());}
    match IndexedDialogVault::open(StateDirectoryLock::acquire(&path)?,owner,config){
        Err(Error::Integrity("anchor version"))=>println!("S3_PREDECESSOR_REFUSED authenticated_anchor_version=true before_writable_api=true"),
        Err(error)=>return Err(format!("wrong refusal/setup error: {error}").into()),
        Ok(mut store)=>{
            let _=store.commit_history(0,store.generation(),[21;32],HistoryOperation::QueueAccept{request:[22;16],text:"forbidden predecessor queue".into()});
            let _=store.commit(store.generation(),[23;32],vec![],vec![]);
            return Err("predecessor admitted RuntimeRoot6 to writable API".into());
        }
    }
    Ok(())
}
