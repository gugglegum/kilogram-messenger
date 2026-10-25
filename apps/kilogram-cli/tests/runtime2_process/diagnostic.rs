//! One bounded attribution probe. It is not P acceptance and changes no scheduler gates.
use super::*;

fn cpu_millis(pid: u32) -> Result<Option<f64>> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let output = Command::new("powershell.exe")
            .creation_flags(0x0800_4000)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                &format!(
                    "$probeProcess=Get-Process -Id {pid}; [Console]::Write($probeProcess.TotalProcessorTime.TotalMilliseconds.ToString([Globalization.CultureInfo]::InvariantCulture))"
                ),
            ])
            .output()?;
        ensure!(output.status.success(), "owned process CPU sample");
        Ok(Some(std::str::from_utf8(&output.stdout)?.trim().parse()?))
    }
    #[cfg(not(windows))]
    {
        let _ = pid;
        Ok(None)
    }
}

#[tokio::test]
async fn a3_s4_diagnostic_owner_costs_64_one_way_duplex_burst() -> Result<()> {
    if !std::env::var("KILOGRAM_TEST_OD2_TIMINGS").is_ok_and(|s| s == "1") {
        println!("S4_DIAGNOSTIC_NOT_SELECTED explicit numerical timing opt-in required");
        return Ok(());
    }
    tokio::time::timeout(Duration::from_secs(280), async {
        let clock = Instant::now();
        let mut pair = Pair::new("diagnostic-costs-").await?;
        let setup = clock.elapsed().as_secs_f64();
        let pids = [
            pair.nodes[0].child.as_ref().context("A child")?.id(),
            pair.nodes[1].child.as_ref().context("B child")?.id(),
        ];
        let cpu_before = [cpu_millis(pids[0])?, cpu_millis(pids[1])?];
        let data_clock = Instant::now();
        for n in 1..=64 {
            pair.nodes[0].send(0, n).await?;
        }
        pair.converged(0, 66).await?;
        let one_way = data_clock.elapsed().as_secs_f64();
        let duplex_clock = Instant::now();
        for n in 65..=66 {
            pair.nodes[0].send(0, n).await?;
        }
        pair.state(
            1,
            Duration::from_secs(90),
            |s| s["contexts"][0]["details"]["native"]["peer_next"] == 67,
            "diagnostic A readable before B burst",
        )
        .await?;
        for n in 1..=32 {
            pair.nodes[1].send(0, n).await?;
        }
        pair.converged(0, 100).await?;
        let duplex = duplex_clock.elapsed().as_secs_f64();
        let data_elapsed = data_clock.elapsed().as_secs_f64();
        let cpu_after = [cpu_millis(pids[0])?, cpu_millis(pids[1])?];
        let history_clock = Instant::now();
        for node in 0..2 {
            pair.history(node, 0, 98).await?;
        }
        let history = history_clock.elapsed().as_secs_f64();
        for node in &mut pair.nodes {
            node.stop(false).await?;
        }
        for node in &pair.nodes {
            let raw = std::fs::read_to_string(node.profile.with_extension("1.stderr.log"))?;
            ensure!(raw.contains("od2_diag "), "diagnostic summary missing");
        }
        println!(
            "S4_DIAGNOSTIC_DONE acceptance_P=false watchdog=280 text=98 genesis=4 setup_seconds={setup:.6} one_way64_seconds={one_way:.6} duplex34_seconds={duplex:.6} data_elapsed_seconds={data_elapsed:.6} history_seconds={history:.6} controller_seconds={:.6} pids={pids:?} cpu_before_ms={cpu_before:?} cpu_after_ms={cpu_after:?}",
            clock.elapsed().as_secs_f64()
        );
        Ok::<_, anyhow::Error>(())
    })
    .await
    .context("bounded diagnostic280s watchdog")?
}
