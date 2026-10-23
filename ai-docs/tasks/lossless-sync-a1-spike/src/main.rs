//! Test-only offline spike. No product code, wire, vault, registry or sockets.
mod f1;
mod f2;

use anyhow::Result;

fn main() -> Result<()> {
    let directory = tempfile::tempdir()?;
    println!("scope=offline-feasibility; olm=0.10.0; synthetic-keys=true");
    f1::run(directory.path())?;
    f2::run(directory.path())?;
    println!("status=passed; production-a1-closed=false");
    Ok(())
}
