use serenity::{
    async_trait,
    client::{Client, Context, EventHandler},
    model::{
        channel::Message,
        gateway::Ready,
        id::{ChannelId, GuildId},
        application::{
            interaction::{Interaction, InteractionResponseType},
            command::{Command, CommandOptionType},
        },
        guild::Role,
        channel::Reaction,
    },
    prelude::GatewayIntents,
};
use reqwest::Client as HttpClient;
use std::env;
use std::sync::Arc;
use rand::seq::SliceRandom;
use serde_json::Value;
use std::path::Path;
use std::collections::HashMap;
use chrono::{Utc, NaiveDate};
use chrono_tz::Europe::Berlin;
use rand::Rng;
use tokio_postgres::{NoTls, types::ToSql};
use bb8::Pool;
use bb8_postgres::PostgresConnectionManager;

struct Handler;

struct Database {
    pool: Arc<Pool<PostgresConnectionManager<NoTls>>>,
}

impl Database {
    async fn new() -> Self {
        let db_url = env::var("DATABASE_URL").expect("Expected DATABASE_URL in the environment");
        let manager = PostgresConnectionManager::new_from_stringlike(db_url, NoTls)
            .expect("Failed to create Postgres manager");
        let pool = Arc::new(Pool::builder()
            .build(manager)
            .await
            .expect("Failed to create database pool"));

        {
            let conn = pool.get().await.expect("Failed to get connection from pool");
            conn.execute(
                "CREATE TABLE IF NOT EXISTS users (
                    user_id BIGINT PRIMARY KEY,
                    nuggets BIGINT NOT NULL DEFAULT 0,
                    last_daily DATE
                )",
                &[],
            ).await.expect("Failed to create users table");
        }

        Database { pool }
    }
}

struct DatabaseKey;
impl serenity::prelude::TypeMapKey for DatabaseKey {
    type Value = Arc<Database>;
}

struct NuggiesPersonality;
impl serenity::prelude::TypeMapKey for NuggiesPersonality {
    type Value = String;
}

async fn handle_reaction_role(ctx: &Context, reaction: &Reaction, add: bool) {
    if reaction.user(&ctx.http).await.map_or(true, |u| u.bot) {
        return;
    }

    if let Ok(msg) = reaction.message(&ctx.http).await {
        if !msg.author.bot {
            return;
        }

        let guild_id = match reaction.guild_id {
            Some(id) => id,
            None => return,
        };
        let user_id = match reaction.user_id {
            Some(id) => id,
            None => return,
        };
        let mut member = match guild_id.member(&ctx.http, user_id).await {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[ERROR] Could not fetch member (ID: {}): {:?}", user_id, e);
                return;
            }
        };

        let emoji_name = if let serenity::model::channel::ReactionType::Custom { name, .. } = &reaction.emoji {
            name.as_deref().unwrap_or("")
        } else {
            ""
        };

        let role_name_to_assign: Option<&str> = if msg.content.starts_with("Assign yourself Pronouns") {
            let roles_map: HashMap<&str, &str> = [
                ("justaboy", "he/him"), ("justagirl", "she/her"), ("pridejj", "they/them"),
            ].iter().cloned().collect();
            roles_map.get(emoji_name).copied()
        } else if msg.content.contains("role for event notifications") && emoji_name == "danseparty" {
            Some("FC Events")
        } else if msg.content.starts_with("Who are you?") {
            let roles_map: HashMap<&str, &str> = [
                ("dodo", "Stinki"), ("lurkk", "FC Member"), ("flowah", "Fren"),
            ].iter().cloned().collect();
            roles_map.get(emoji_name).copied()
        } else {
            None
        };

        if let Some(role_name) = role_name_to_assign {
            if let Some(role) = guild_id.roles(&ctx.http).await.unwrap().values().find(|r| r.name == role_name) {
                let action_result = if add {
                    member.add_role(&ctx.http, role.id).await
                } else {
                    member.remove_role(&ctx.http, role.id).await
                };

                match action_result {
                    Ok(_) => println!("[SUCCESS] Role action updated for {}", member.user.name),
                    Err(e) => eprintln!("[ERROR] Failed role action: {:?}", e),
                }
            }
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _ctx: Context, ready: Ready) {
        println!("[INFO] Bot is connected as {} (ID: {})", ready.user.name, ready.user.id);

        let patch_channel_id = ChannelId(1412130150325289203);
        let today_date = Utc::now().with_timezone(&Berlin).format("%Y-%m-%d").to_string();

        let patch_notes = format!(
            "**Patch Notes - {}**\n\n\
            - Improved AI response length management\n\
            - Added character limit safety check to prevent message failures",
            today_date
        );

        let _ = patch_channel_id.say(&_ctx.http, &patch_notes).await;

        let _ = Command::set_global_application_commands(&_ctx.http, |commands| {
            commands
                .create_application_command(|command| {
                    command.name("nuggies").description("Chat with Nuggies AI")
                        .create_option(|option| {
                            option.name("message").description("Your message").kind(CommandOptionType::String).required(true)
                        })
                })
                .create_application_command(|command| {
                    command.name("ask").description("Ask the AI a question")
                        .create_option(|option| {
                            option.name("question").description("Your question").kind(CommandOptionType::String).required(true)
                        })
                })
                .create_application_command(|command| command.name("fox").description("Get a random fox GIF"))
                .create_application_command(|command| {
                    command.name("translate").description("Translate text")
                        .create_option(|option| { option.name("language").description("Language").kind(CommandOptionType::String).required(true) })
                        .create_option(|option| { option.name("text").description("Text").kind(CommandOptionType::String).required(true) })
                })
                .create_application_command(|command| command.name("daily").description("Claim daily nuggets"))
                .create_application_command(|command| command.name("nuggetbox").description("Check nuggets"))
                .create_application_command(|command| command.name("leaderboard").description("Top nugget holders"))
                .create_application_command(|command| {
                    command.name("slots").description("Play slots")
                        .create_option(|option| { option.name("amount").description("Bet amount").kind(CommandOptionType::Integer).min_int_value(1).max_int_value(10) })
                })
                .create_application_command(|command| {
                    command.name("gift").description("Gift nuggies")
                        .create_option(|option| { option.name("amount").description("Amount").kind(CommandOptionType::Integer).required(true).min_int_value(1) })
                        .create_option(|option| { option.name("user").description("User").kind(CommandOptionType::User).required(true) })
                })
                .create_application_command(|command| command.name("help").description("Show commands"))
        }).await;
    }

    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot { return; }

        let lower_content = msg.content.to_lowercase();
        if lower_content.contains("istanbul") {
            let image_path = Path::new("constantinople.png");
            if image_path.exists() {
                let _ = msg.channel_id.send_files(&ctx.http, vec![image_path], |m| m.content("That's Constantinople!")).await;
            }
        } else if lower_content.contains("nuggies") {
            let data = ctx.data.read().await;
            let mistral_api_key = data.get::<MistralApiKey>().unwrap().clone();
            let personality_prompt = data.get::<NuggiesPersonality>().unwrap().clone();
            
            let prompt = format!(
                "{}\nRespond briefly (1-2 sentences) to: {}",
                personality_prompt, &msg.content
            );
            
            if let Ok(mut response) = call_mistral_api(&mistral_api_key, &prompt).await {
                if response.len() > 2000 { response.truncate(1997); response.push_str("..."); }
                let _ = msg.channel_id.say(&ctx.http, &response).await;
            }
        }
    }

    async fn reaction_add(&self, ctx: Context, reaction: Reaction) { handle_reaction_role(&ctx, &reaction, true).await; }
    async fn reaction_remove(&self, ctx: Context, reaction: Reaction) { handle_reaction_role(&ctx, &reaction, false).await; }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Some(command) = interaction.application_command() {
            let _ = command.create_interaction_response(&ctx.http, |response| {
                response.kind(InteractionResponseType::DeferredChannelMessageWithSource)
            }).await;

            let user_id = command.user.id;
            let command_name = command.data.name.clone();
            let ctx_clone = ctx.clone();

            tokio::spawn(async move {
                let data = ctx_clone.data.read().await;
                let mut response_content = match command_name.as_str() {
                    "nuggies" => {
                        let message_text = command.data.options.iter().find(|opt| opt.name == "message").and_then(|opt| opt.value.as_ref()?.as_str()).unwrap_or("");
                        let mistral_api_key = data.get::<MistralApiKey>().unwrap().clone();
                        let personality_prompt = data.get::<NuggiesPersonality>().unwrap().clone();
                        let prompt = format!("{}\nRespond to this: {}", personality_prompt, message_text);
                        match call_mistral_api(&mistral_api_key, &prompt).await {
                            Ok(res) => format!("<@{}> asked: {}\n\n{}", user_id.0, message_text, res),
                            Err(_) => "Error connecting to AI.".to_string(),
                        }
                    },
                    "ask" => {
                        let question_text = command.data.options.iter().find(|opt| opt.name == "question").and_then(|opt| opt.value.as_ref()?.as_str()).unwrap_or("");
                        let mistral_api_key = data.get::<MistralApiKey>().unwrap().clone();
                        // UPDATED PROMPT: More descriptive length instruction
                        let prompt = format!(
                            "Question: {}\n\nInstruction: Provide a concise, short-to-medium length answer. Be direct and avoid unnecessary fluff.", 
                            question_text
                        );
                        let response = call_mistral_api(&mistral_api_key, &prompt).await.unwrap_or_else(|_| "Error.".to_string());
                        format!("<@{}> asked: {}\n\n{}", user_id.0, question_text, response)
                    },
                    "fox" => {
                        let tenor_api_key = data.get::<TenorApiKey>().unwrap().clone();
                        get_random_fox_gif(&tenor_api_key).await.unwrap_or_else(|_| "https://media.tenor.com/YxT1w3VX5BAAAAAM/fox-dance.gif".to_string())
                    },
                    "daily" => {
                        let db = data.get::<DatabaseKey>().unwrap();
                        let conn = db.pool.get().await.expect("Failed to get DB connection");
                        let user_id_i64 = *user_id.as_u64() as i64;
                        let today = Utc::now().with_timezone(&Berlin).date_naive();
                        let row_opt = conn.query_one("SELECT nuggets, last_daily FROM users WHERE user_id = $1", &[&user_id_i64]).await.ok();

                        if let Some(row) = row_opt {
                            let nuggets: i64 = row.get(0);
                            let last_daily: Option<NaiveDate> = row.get(1);
                            if last_daily == Some(today) {
                                "Try again tomorrow!".to_string()
                            } else {
                                let gain: i64 = rand::thread_rng().gen_range(1..=25);
                                conn.execute("UPDATE users SET nuggets = $1, last_daily = $2 WHERE user_id = $3", &[&(nuggets + gain), &today, &user_id_i64]).await.unwrap();
                                format!("You received {} nuggets!", gain)
                            }
                        } else {
                            let gain: i64 = rand::thread_rng().gen_range(1..=15);
                            conn.execute("INSERT INTO users (user_id, nuggets, last_daily) VALUES ($1, $2, $3)", &[&user_id_i64, &gain, &today]).await.unwrap();
                            format!("Welcome! You received {} nuggets!", gain)
                        }
                    },
                    "nuggetbox" => {
                        let db = data.get::<DatabaseKey>().unwrap();
                        let conn = db.pool.get().await.unwrap();
                        if let Ok(row) = conn.query_one("SELECT nuggets FROM users WHERE user_id = $1", &[&(*user_id.as_u64() as i64)]).await {
                            format!("You have {} nuggets.", row.get::<_, i64>(0))
                        } else { "Use `/daily` first!".to_string() }
                    },
                    "slots" => {
                        let db = data.get::<DatabaseKey>().unwrap();
                        let conn = db.pool.get().await.unwrap();
                        let user_id_i64 = *user_id.as_u64() as i64;
                        let bet = command.data.options.iter().find(|o| o.name == "amount").and_then(|o| o.value.as_ref()?.as_i64()).unwrap_or(5);

                        if let Ok(row) = conn.query_one("SELECT nuggets FROM users WHERE user_id = $1", &[&user_id_i64]).await {
                            let current: i64 = row.get(0);
                            if current < bet { "Not enough nuggets!".to_string() }
                            else {
                                let mut rng = rand::thread_rng();
                                let roll = rng.gen_range(1..=100);
                                let win = if roll <= 10 { bet * 5 } else if roll <= 30 { bet } else { 0 };
                                conn.execute("UPDATE users SET nuggets = $1 WHERE user_id = $2", &[&(current - bet + win), &user_id_i64]).await.unwrap();
                                if win > bet { format!("WIN! You got {} nuggets!", win) } else if win == bet { "Break even!".to_string() } else { "Lost!".to_string() }
                            }
                        } else { "Use `/daily` first!".to_string() }
                    },
                    "help" => "Commands: /nuggies, /ask, /fox, /translate, /daily, /nuggetbox, /leaderboard, /slots, /gift".to_string(),
                    _ => "Unknown command.".to_string(),
                };

                // SAFETY CHECK: Ensure we never exceed Discord's 2000 character limit
                if response_content.len() > 2000 {
                    response_content.truncate(1997);
                    response_content.push_str("...");
                }

                let _ = command.edit_original_interaction_response(&ctx_clone.http, |res| res.content(response_content)).await;
            });
        }
    }
}

async fn get_or_create_role(ctx: &Context, guild_id: GuildId, role_name: &str) -> Option<Role> {
    let roles = guild_id.roles(&ctx.http).await.ok()?;
    if let Some(role) = roles.values().find(|r| r.name == role_name) { return Some(role.clone()); }
    guild_id.create_role(&ctx.http, |r| r.name(role_name).mentionable(true)).await.ok()
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    let discord_token = env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN required");
    let mistral_api_key = env::var("MISTRAL_API_KEY").expect("MISTRAL_API_KEY required");
    let tenor_api_key = env::var("TENOR_API_KEY").expect("TENOR_API_KEY required");
    let nuggies_personality = env::var("NUGGIES_PERSONALITY").unwrap_or_else(|_| "You are Nuggies, a witty assistant.".to_string());

    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT | GatewayIntents::GUILD_MESSAGE_REACTIONS | GatewayIntents::GUILDS | GatewayIntents::GUILD_MEMBERS;

    let mut client = Client::builder(discord_token, intents).event_handler(Handler).await.expect("Error creating client");

    {
        let mut data = client.data.write().await;
        data.insert::<MistralApiKey>(Arc::new(mistral_api_key));
        data.insert::<TenorApiKey>(Arc::new(tenor_api_key));
        data.insert::<NuggiesPersonality>(nuggies_personality);
        data.insert::<DatabaseKey>(Arc::new(Database::new().await));
    }

    if let Err(why) = client.start().await { println!("Client error: {:?}", why); }
}

struct MistralApiKey;
impl serenity::prelude::TypeMapKey for MistralApiKey { type Value = Arc<String>; }
struct TenorApiKey;
impl serenity::prelude::TypeMapKey for TenorApiKey { type Value = Arc<String>; }

async fn call_mistral_api(api_key: &str, prompt: &str) -> Result<String, reqwest::Error> {
    let client = HttpClient::new();
    let request_body = serde_json::json!({
        "model": "mistral-tiny",
        "messages": [{"role": "user", "content": prompt}],
        "temperature": 0.7,
    });

    let response = client.post("https://api.mistral.ai/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request_body).send().await?;

    let response_json = response.json::<serde_json::Value>().await?;
    let content = response_json["choices"][0]["message"]["content"].as_str().unwrap_or("I couldn't think of anything.");
    Ok(content.to_string())
}

async fn get_random_fox_gif(api_key: &str) -> Result<String, reqwest::Error> {
    let client = HttpClient::new();
    let url = format!("https://tenor.googleapis.com/v2/search?q=fox&key={}&limit=20", api_key);
    let response = client.get(&url).send().await?;
    let response_json: Value = response.json().await?;
    let gifs = response_json["results"].as_array().unwrap();
    let random_gif = gifs.choose(&mut rand::thread_rng()).unwrap()["media_formats"]["gif"]["url"].as_str().unwrap().to_string();
    Ok(random_gif)
}
