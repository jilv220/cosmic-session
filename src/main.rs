// SPDX-License-Identifier: GPL-3.0-only
#[macro_use]
extern crate tracing;

mod comp;
mod generic;
mod process;

use async_signals::Signals;
use color_eyre::{eyre::WrapErr, Result};
use futures_util::StreamExt;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::metadata::LevelFilter;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
	color_eyre::install().wrap_err("failed to install color_eyre error handler")?;

	tracing_subscriber::registry()
		.with(fmt::layer())
		.with(
			EnvFilter::builder()
				.with_default_directive(LevelFilter::INFO.into())
				.from_env_lossy(),
		)
		.try_init()
		.wrap_err("failed to initialize logger")?;

	info!("Starting cosmic-session");

	let token = CancellationToken::new();
	let (socket_tx, socket_rx) = mpsc::unbounded_channel();
	if let Err(err) = comp::run_compositor(token.child_token(), socket_rx) {
		error!("compositor errored: {:?}", err);
	}
	tokio::time::sleep(std::time::Duration::from_millis(100)).await;
	let env_vars = Vec::new();
	info!("got environmental variables: {:?}", env_vars);

	let mut sockets = Vec::with_capacity(2);

	generic::run_executable(
		token.child_token(),
		info_span!(parent: None, "cosmic-panel"),
		"cosmic-panel",
		vec!["testing-panel".into()],
		comp::create_privileged_socket(&mut sockets, &env_vars)
			.wrap_err("failed to create panel socket")?,
	);
	generic::run_executable(
		token.child_token(),
		info_span!(parent: None, "cosmic-panel dock"),
		"cosmic-panel",
		vec!["testing-dock".into()],
		comp::create_privileged_socket(&mut sockets, &env_vars)
			.wrap_err("failed to create dock socket")?,
	);
	generic::run_executable(
		token.child_token(),
		info_span!(parent: None, "cosmic-app-library"),
		"cosmic-app-library",
		vec![],
		comp::create_privileged_socket(&mut sockets, &env_vars)
			.wrap_err("failed to create dock socket")?,
	);

	socket_tx.send(sockets).unwrap();

	let mut signals = Signals::new(vec![libc::SIGTERM, libc::SIGINT]).unwrap();
	while let Some(signal) = signals.next().await {
		match signal {
			libc::SIGTERM | libc::SIGINT => {
				info!("received request to terminate");
				token.cancel();
				tokio::time::sleep(std::time::Duration::from_secs(2)).await;
				break;
			}
			_ => unreachable!("received unhandled signal {}", signal),
		}
	}

	Ok(())
}
