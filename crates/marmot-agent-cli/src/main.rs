use clap::{Parser, Subcommand};
use tracing::Level;

#[derive(Parser)]
#[command(name = "marmot-cli")]
#[command(about = "A headless Marmot messaging agent, inspired by signal-cli.")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long, global = true, help = "Increase verbosity")]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a new Nostr identity or list existing ones.
    Identity {
        #[command(subcommand)]
        action: IdentityAction,
    },
    /// Manage relay connections.
    Relay {
        #[command(subcommand)]
        action: RelayAction,
    },
    Daemon {
        #[arg(short, long, default_value = "/tmp/marmot-agent.sock")]
        listen: String,
    },
    Groups,
}

#[derive(Subcommand)]
enum IdentityAction {
    Create {
        #[arg(short, long, help = "Human-readable name")]
        name: Option<String>,
    },
    List,
    Show {
        #[arg(help = "Public key (hex or npub)")]
        pubkey: Option<String>,
    },
}

#[derive(Subcommand)]
enum RelayAction {
    List,
    Add { url: String },
}

fn main() {
    let cli = Cli::parse();

    let level = if cli.verbose { Level::DEBUG } else { Level::INFO };
    tracing_subscriber::fmt()
        .with_max_level(level)
        .init();

    match cli.command {
        Commands::Identity { action } => match action {
            IdentityAction::Create { name } => {
                let id = if let Some(n) = name {
                    marmot_agent_core::identity::Identity::generate_named(n)
                } else {
                    marmot_agent_core::identity::Identity::generate()
                };
                println!("Identity created");
                println!("  npub: {}", id.npub());
                println!("  nsec: {}", id.nsec());
            }
            IdentityAction::List => {
                println!("Identity listing not yet implemented");
            }
            IdentityAction::Show { pubkey: _ } => {
                println!("Identity show not yet implemented");
            }
        },
        Commands::Relay { action } => match action {
            RelayAction::List => {
                println!("Default relays:");
                for url in marmot_agent_core::relay::DEFAULT_RELAYS {
                    println!("  {}", url);
                }
            }
            RelayAction::Add { url } => {
                println!("Adding relay {}...", url);
            }
        },
        Commands::Daemon { listen } => {
            println!("Starting daemon on {} (not yet implemented)", listen);
        }
        Commands::Groups => {
            println!("Groups listing not yet implemented");
        }
    }
}
