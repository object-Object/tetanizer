mod schema;

extern crate dotenv;

use dotenv::dotenv;
use serenity::{async_trait, model::gateway::Ready, prelude::*};
use std::{env, fs, path::Path};
use tantivy::{directory::MmapDirectory, schema::Schema, Index};

use crate::schema::MessageSchema;

fn open_index<P>(path: P, schema: Schema) -> Index
where
    P: AsRef<Path> + Copy,
{
    fs::create_dir_all(path).expect("Failed to create index directory");
    let directory = MmapDirectory::open(path).expect("Failed to open index directory");
    Index::open_or_create(directory, schema).expect("Failed to open index")
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("Logged in as {}", ready.user.name);

        let schema = MessageSchema::build();
        let index = open_index("./index", schema.clone_inner());
    }
}

#[tokio::main]
async fn main() {
    // read environment variables
    dotenv().ok();
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    // create client
    let intents =
        GatewayIntents::GUILDS | GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(token, intents)
        .event_handler(Handler)
        .await
        .expect("Error creating client");

    // run the bot
    if let Err(oh_no) = client.start().await {
        println!("Client error: {oh_no:?}");
    }
}
