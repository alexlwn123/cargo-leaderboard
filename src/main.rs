use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    match cargo_leaderboard::run().await {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::from(1)
        }
    }
}
