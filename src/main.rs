use serenity::{
    async_trait,
    client::{Client, Context, EventHandler},
    model::{
        channel::Message,
        gateway::Ready,
        id::GuildId,
        application::interaction::{Interaction, InteractionResponseType},
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
                eprintln!("[ERROR] Could not fetch member: {:?}", e);
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
        } else {
            None
        };

        if let Some(role_name) = role_name_to_assign {
            println!("[REACTION] User '{}' reacted with '{}' for role '{}'.", member.user.name, emoji_name, role_name);
            if let Some(role) = guild_id.roles(&ctx.http).await.unwrap().values().find(|r| r.name == role_name) {
                let action_result = if add {
                    member.add_role(&ctx.http, role.id).await
                } else {
                    member.remove_role(&ctx.http, role.id).await
                };

                let action_str = if add { "Assigned" } else { "Removed" };
                let action_str_fail = if add { "assign" } else { "remove" };

                if action_result.is_ok() {
                    println!("[SUCCESS] {} role '{}' {} '{}'.", action_str, role.name, if add {"to"} else {"from"}, member.user.name);
                } else {
                    eprintln!("[ERROR] Failed to {} role '{}' {} '{}'. Check permissions.", action_str_fail, role.name, if add {"to"} else {"from"}, member.user.name);
                }
            }
        }
    }
}


#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _ctx: Context, ready: Ready) {
        println!("[INFO] Bot is connected as {}", ready.user.name);

        let commands = serenity::model::application::command::Command::set_global_application_commands(&_ctx.http, |commands| {
            commands
                .create_application_command(|command| {
                    command.name("nuggies").description("Chat with Nuggies AI")
                        .create_option(|option| {
                            option.name("message")
                                .description("Your message to Nuggies")
                                .kind(serenity::model::application::command::CommandOptionType::String)
                                .required(true)
                        })
                })
                .create_application_command(|command| {
                    command.name("ask").description("Ask the AI a question")
                        .create_option(|option| {
                            option.name("question")
                                .description("Your question for the AI")
                                .kind(serenity::model::application::command::CommandOptionType::String)
                                .required(true)
                        })
                })
                .create_application_command(|command| {
                    command.name("fox").description("Get a random fox GIF")
                })
                .create_application_command(|command| {
                    command.name("translate").description("Translate text to a specified language")
                        .create_option(|option| {
                            option.name("language")
                                .description("The language to translate to (e.g., 'French')")
                                .kind(serenity::model::application::command::CommandOptionType::String)
                                .required(true)
                        })
                        .create_option(|option| {
                            option.name("text")
                                .description("The text to translate")
                                .kind(serenity::model::application::command::CommandOptionType::String)
                                .required(true)
                        })
                })
                .create_application_command(|command| {
                    command.name("daily").description("Claim your daily nuggets")
                })
                .create_application_command(|command| {
                    command.name("nuggetbox").description("Check your personal amount of nuggets")
                })
                .create_application_command(|command| {
                    command.name("slots").description("Spend 5 nuggets for a chance to win big!")
                })
        })
            .await;

        if let Err(e) = commands {
            eprintln!("[ERROR] Error creating global application commands: {:?}", e);
        } else {
            println!("[INFO] Successfully registered global application commands!");
        }
    }

    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot {
            return;
        }

        if msg.author.id.0 == 241614046913101825 && msg.content == "assignrole:gender" {
            println!("[CMD] Triggered 'assignrole:gender' by authorized user.");
            let guild_id = msg.guild_id.unwrap();

            let role_names = ["he/him", "she/her", "they/them"];
            let emoji_names = ["justaboy", "justagirl", "pridejj"];

            println!("[DEBUG] Verifying roles exist...");
            for role_name in role_names.iter() {
                if get_or_create_role(&ctx, guild_id, role_name).await.is_none() {
                    eprintln!("[ERROR] Failed to get or create role: {}. Aborting.", role_name);
                    return;
                }
            }
            println!("[DEBUG] Role verification complete.");

            println!("[DEBUG] Fetching custom emojis...");
            let guild_emojis = match guild_id.emojis(&ctx.http).await {
                Ok(emojis) => emojis,
                Err(e) => {
                    eprintln!("[ERROR] Could not fetch emojis for guild: {:?}. Aborting.", e);
                    return;
                }
            };

            let mut emojis = Vec::new();
            for name in &emoji_names {
                if let Some(emoji) = guild_emojis.iter().find(|e| e.name == *name) {
                    emojis.push(emoji.clone());
                } else {
                    eprintln!("[ERROR] Could not find emoji '{}' on the server. Aborting.", name);
                    return;
                }
            }
            println!("[DEBUG] Emojis fetched successfully.");

            let message_content = format!(
                "Assign yourself Pronouns\n{} He/Him\n{} She/Her\n{} They/Them",
                emojis[0], emojis[1], emojis[2]
            );

            if let Ok(sent_message) = msg.channel_id.say(&ctx.http, &message_content).await {
                println!("[ACTION] Successfully sent role assignment message.");
                for emoji in emojis {
                    let _ = sent_message.react(&ctx.http, emoji).await;
                }
            } else {
                eprintln!("[ERROR] Failed to send role assignment message.");
            }

            let _ = msg.delete(&ctx.http).await;
            return;
        }
        else if msg.author.id.0 == 241614046913101825 && msg.content == "assignrole:fcevents" {
            println!("[CMD] Triggered 'assignrole:fcevents' by authorized user.");
            let guild_id = msg.guild_id.unwrap();

            let role_name = "FC Events";
            let emoji_name = "danseparty";

            if get_or_create_role(&ctx, guild_id, role_name).await.is_none() {
                eprintln!("[ERROR] Failed to get or create role: '{}'. Aborting.", role_name);
                return;
            }

            let guild_emojis = match guild_id.emojis(&ctx.http).await {
                Ok(emojis) => emojis,
                Err(e) => {
                    eprintln!("[ERROR] Could not fetch emojis for guild: {:?}. Aborting.", e);
                    return;
                }
            };

            if let Some(emoji) = guild_emojis.iter().find(|e| e.name == emoji_name) {
                let message_content = format!(
                    "React with {} to get the '{}' role for event notifications!",
                    emoji, role_name
                );

                if let Ok(sent_message) = msg.channel_id.say(&ctx.http, &message_content).await {
                    println!("[ACTION] Successfully sent role assignment message for FC Events.");
                    if let Err(e) = sent_message.react(&ctx.http, emoji.clone()).await {
                        eprintln!("[ERROR] Failed to react to the message: {:?}", e);
                    }
                } else {
                    eprintln!("[ERROR] Failed to send role assignment message for FC Events.");
                }
            } else {
                eprintln!("[ERROR] Could not find emoji ':{}:' on the server. Aborting.", emoji_name);
                return;
            }

            let _ = msg.delete(&ctx.http).await;
            return;
        }

        let lower_content = msg.content.to_lowercase();
        if lower_content.contains("istanbul") {
            println!("[CMD] Triggered 'istanbul' response.");
            let image_path = Path::new("constantinople.png");
            if image_path.exists() {
                let _ = msg.channel_id.send_files(&ctx.http, vec![image_path], |m| m.content("That's Constantinople!")).await;
            } else {
                let _ = msg.channel_id.say(&ctx.http, "That's Constantinople! (but I couldn't find the image)").await;
            }
        } else if lower_content.contains("nuggies") {
            println!("[CMD] Triggered 'nuggies' AI response.");
            let typing = msg.channel_id.start_typing(&ctx.http);
            let data = ctx.data.read().await;
            let gemini_api_key = data.get::<GeminiApiKey>().expect("Expected GeminiApiKey in TypeMap.").clone();
            let personality_prompt = get_nuggies_personality_prompt();
            let modified_prompt = format!(
                "{}\nRespond to the following message as Nuggies and keep the response at one or 2 sentences:\n\n{}",
                personality_prompt, &msg.content
            );
            let response = call_gemini_api(&gemini_api_key, &modified_prompt).await.unwrap_or_else(|_| "My circuits are fried.".to_string());
            let _ = typing.map(|t| t.stop());
            let _ = msg.channel_id.say(&ctx.http, &response).await;
        }
    }

    async fn reaction_add(&self, ctx: Context, reaction: Reaction) {
        handle_reaction_role(&ctx, &reaction, true).await;
    }

    async fn reaction_remove(&self, ctx: Context, reaction: Reaction) {
        handle_reaction_role(&ctx, &reaction, false).await;
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Some(command) = interaction.application_command() {
            println!("[SLASH CMD] Received command: '/{}'.", command.data.name);

            let _ = command.create_interaction_response(&ctx.http, |response| {
                response.kind(InteractionResponseType::DeferredChannelMessageWithSource)
            }).await;

            let user_id = command.user.id;
            let command_name = command.data.name.clone();
            let ctx_clone = ctx.clone();

            tokio::spawn(async move {
                let response_content = match command_name.as_str() {
                    "nuggies" => {
                        let message_option = command.data.options.iter().find(|opt| opt.name == "message");
                        if let Some(message_text) = message_option.and_then(|opt| opt.value.as_ref().and_then(|v| v.as_str())) {
                            let data = ctx_clone.data.read().await;
                            let gemini_api_key = data.get::<GeminiApiKey>().unwrap().clone();
                            let personality_prompt = get_nuggies_personality_prompt();
                            let prompt = format!(
                                "{}\nRespond to the following message as Nuggies:\n\n{}",
                                personality_prompt, message_text
                            );
                            match call_gemini_api(&gemini_api_key, &prompt).await {
                                Ok(response) => format!("<@{}> asked: {}\n\n{}", user_id.0, message_text, response),
                                Err(_) => "Sorry, I couldn't get a response from Nuggies right now.".to_string(),
                            }
                        } else { "Please provide a message for Nuggies.".to_string() }
                    },
                    "ask" => {
                        let question_option = command.data.options.iter().find(|opt| opt.name == "question");
                        if let Some(question_text) = question_option.and_then(|opt| opt.value.as_ref().and_then(|v| v.as_str())) {
                            let data = ctx_clone.data.read().await;
                            let gemini_api_key = data.get::<GeminiApiKey>().unwrap().clone();
                            let response = call_gemini_api(&gemini_api_key, question_text).await.unwrap_or_else(|_| "Sorry, I couldn't get a response right now.".to_string());
                            format!("<@{}> asked: {}\n\n{}", user_id.0, question_text, response)
                        } else { "Please provide a question.".to_string() }
                    },
                    "translate" => {
                        let lang_opt = command.data.options.iter().find(|o| o.name == "language").and_then(|o| o.value.as_ref().and_then(|v| v.as_str()));
                        let text_opt = command.data.options.iter().find(|o| o.name == "text").and_then(|o| o.value.as_ref().and_then(|v| v.as_str()));

                        if let (Some(language), Some(text)) = (lang_opt, text_opt) {
                            let data = ctx_clone.data.read().await;
                            let gemini_api_key = data.get::<GeminiApiKey>().unwrap().clone();
                            let prompt = format!("Translate the following text to {} exactly and only output the translated text:\n\n{}", language, text);
                            call_gemini_api(&gemini_api_key, &prompt).await.unwrap_or_else(|_| "Sorry, I couldn't translate that.".to_string())
                        } else { "Please provide both a language and text.".to_string() }
                    },
                    "fox" => {
                        let data = ctx_clone.data.read().await;
                        let tenor_api_key = data.get::<TenorApiKey>().unwrap().clone();
                        get_random_fox_gif(&tenor_api_key).await.unwrap_or_else(|_| "https://media.tenor.com/YxT1w3VX5BAAAAAM/fox-dance.gif".to_string())
                    },
                    "daily" => {
                        let data = ctx_clone.data.read().await;
                        let db = data.get::<DatabaseKey>().unwrap();
                        let conn = db.pool.get().await.expect("Failed to get DB connection");
                        let user_id_i64 = *user_id.as_u64() as i64;
                        let today = Utc::now().with_timezone(&Berlin).date_naive();

                        let params: &[&(dyn ToSql + Sync)] = &[&user_id_i64];
                        let row_opt = conn.query_one("SELECT nuggets, last_daily FROM users WHERE user_id = $1", params).await.ok();

                        if let Some(row) = row_opt {
                            let nuggets: i64 = row.get(0);
                            let last_daily: Option<NaiveDate> = row.get(1);

                            if last_daily == Some(today) {
                                "You have already claimed your daily nuggets. Please try again tomorrow.".to_string()
                            } else {
                                let daily_nuggets: i64 = rand::thread_rng().gen_range(1..=15);
                                let new_total = nuggets + daily_nuggets;
                                let update_params: &[&(dyn ToSql + Sync)] = &[&new_total, &today, &user_id_i64];
                                conn.execute("UPDATE users SET nuggets = $1, last_daily = $2 WHERE user_id = $3", update_params).await.unwrap();
                                format!("You received {} nuggets! You now have a total of {} nuggets.", daily_nuggets, new_total)
                            }
                        } else {
                            let daily_nuggets: i64 = rand::thread_rng().gen_range(1..=15);
                            let insert_params: &[&(dyn ToSql + Sync)] = &[&user_id_i64, &daily_nuggets, &today];
                            conn.execute("INSERT INTO users (user_id, nuggets, last_daily) VALUES ($1, $2, $3)", insert_params).await.unwrap();
                            format!("Welcome! You received your first {} nuggets!", daily_nuggets)
                        }
                    },
                    "nuggetbox" => {
                        let data = ctx_clone.data.read().await;
                        let db = data.get::<DatabaseKey>().unwrap();
                        let conn = db.pool.get().await.expect("Failed to get DB connection");
                        let user_id_i64 = *user_id.as_u64() as i64;

                        if let Ok(row) = conn.query_one("SELECT nuggets FROM users WHERE user_id = $1", &[&user_id_i64]).await {
                            let nuggets: i64 = row.get(0);
                            format!("You have {} nuggets in your nuggetbox.", nuggets)
                        } else {
                            "You don't have a nuggetbox yet! Use `/daily` to get your first nuggets.".to_string()
                        }
                    },
                    "slots" => {
                        let data = ctx_clone.data.read().await;
                        let db = data.get::<DatabaseKey>().unwrap();
                        let conn = db.pool.get().await.expect("Failed to get DB connection");
                        let user_id_i64 = *user_id.as_u64() as i64;

                        if let Ok(row) = conn.query_one("SELECT nuggets FROM users WHERE user_id = $1", &[&user_id_i64]).await {
                            let nuggets: i64 = row.get(0);
                            if nuggets < 5 {
                                "You don't have enough nuggets to play the slots! You need at least 5.".to_string()
                            } else {
                                let roll = rand::thread_rng().gen_range(1..=100);
                                let winnings = match roll {
                                    1 => 1000, 2..=3 => 250, 4..=6 => 150, 7..=10 => 100,
                                    11..=15 => 50, 16..=20 => 25, 21..=30 => 10, 31..=50 => 6,
                                    51..=70 => 5, 71..=80 => 4, 81..=90 => 3, 91..=95 => 2,
                                    _ => 1,
                                };
                                let new_total = nuggets - 5 + winnings;
                                let params: &[&(dyn ToSql + Sync)] = &[&new_total, &user_id_i64];
                                conn.execute("UPDATE users SET nuggets = $1 WHERE user_id = $2", params).await.unwrap();

                                let witty_responses = [
                                    "Don't spend it all in one place... or do, I'm not your mother.",
                                    "Fortune favors the bold. Or in your case, the lucky.",
                                    "The gods have smiled upon you. Or perhaps they just sneezed.",
                                    "Ooh, shiny! A gift from my hoard to yours.",
                                    "I suppose that's better than a kick in the teeth.",
                                    "You call that a win? Adorable.",
                                    "Jackpot! Or, you know, a minor financial gain.",
                                    "There. Are you happy now?",
                                ];
                                let witty_response = witty_responses.choose(&mut rand::thread_rng()).unwrap_or(&"");

                                format!(
                                    "You spent 5 nuggets and won {} nuggets! Your new total is {}.\n*{}*",
                                    winnings, new_total, witty_response
                                )
                            }
                        } else {
                            "You don't have a nuggetbox yet! Use `/daily` to get your first nuggets.".to_string()
                        }
                    },
                    _ => "Unknown command.".to_string(),
                };

                if let Err(e) = command.edit_original_interaction_response(&ctx_clone.http, |response| {
                    response.content(response_content)
                }).await {
                    eprintln!("[ERROR] Could not edit interaction response: {:?}", e);
                }
            });
        }
    }
}

async fn get_or_create_role(ctx: &Context, guild_id: GuildId, role_name: &str) -> Option<Role> {
    let roles = guild_id.roles(&ctx.http).await.ok()?;

    if let Some(role) = roles.values().find(|r| r.name == role_name) {
        println!("[DEBUG] Found existing role: '{}'.", role_name);
        return Some(role.clone());
    }

    println!("[ACTION] Role '{}' not found. Creating it now...", role_name);
    match guild_id.create_role(&ctx.http, |r| r.name(role_name).mentionable(true)).await {
        Ok(role) => {
            println!("[SUCCESS] Created role: '{}'.", role.name);
            Some(role)
        },
        Err(e) => {
            eprintln!("[ERROR] Could not create role '{}': {:?}", role_name, e);
            None
        }
    }
}

fn get_nuggies_personality_prompt() -> &'static str {
    "You are an Female AI assistant called 'Nuggies'.\
     You have a somewhat friendly, norse nordic, slightly pagan, with a healthy dose of cute sarcasm, gothic and somewhat unhinged personality.\
     dont Roleplay"
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    let discord_token = env::var("DISCORD_TOKEN").expect("Expected DISCORD_TOKEN in the environment");
    let gemini_api_key = env::var("GEMINI_API_KEY").expect("Expected GEMINI_API_KEY in the environment");
    let tenor_api_key = env::var("TENOR_API_KEY").expect("Expected TENOR_API_KEY in the environment");

    let intents = GatewayIntents::non_privileged()
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILD_MESSAGE_REACTIONS
        | GatewayIntents::GUILDS
        | GatewayIntents::GUILD_MEMBERS;

    let mut client = Client::builder(discord_token, intents)
        .event_handler(Handler)
        .await
        .expect("Error creating client");

    {
        let mut data = client.data.write().await;
        data.insert::<GeminiApiKey>(Arc::new(gemini_api_key));
        data.insert::<TenorApiKey>(Arc::new(tenor_api_key));
        data.insert::<DatabaseKey>(Arc::new(Database::new().await));
    }

    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }
}

struct GeminiApiKey;
impl serenity::prelude::TypeMapKey for GeminiApiKey {
    type Value = Arc<String>;
}

struct TenorApiKey;
impl serenity::prelude::TypeMapKey for TenorApiKey {
    type Value = Arc<String>;
}

async fn call_gemini_api(api_key: &str, message: &str) -> Result<String, reqwest::Error> {
    let client = HttpClient::new();
    let url = "https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash:generateContent";
    let request_body = serde_json::json!({ "contents": [{ "parts": [{ "text": message }] }] });
    let response = client.post(url).header("x-goog-api-key", api_key).json(&request_body).send().await?;
    let response_json = response.json::<serde_json::Value>().await?;
    let response_text = response_json["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .unwrap_or("Sorry, the Endpoint is currently overloaded, please try again.")
        .to_string();
    Ok(response_text)
}

async fn get_random_fox_gif(api_key: &str) -> Result<String, reqwest::Error> {
    let client = HttpClient::new();
    let url = format!("https://tenor.googleapis.com/v2/search?q=fox&key={}&limit=50", api_key);
    let response = client.get(&url).send().await?;
    let response_json: Value = response.json().await?;
    let gifs = response_json["results"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|gif| gif["media_formats"]["gif"]["url"].as_str().map(|s| s.to_string()))
        .collect::<Vec<String>>();
    let mut rng = rand::thread_rng();
    let random_gif = gifs.choose(&mut rng).unwrap_or(&"https://media.tenor.com/YxT1w3VX5BAAAAAM/fox-dance.gif".to_string()).to_string();
    Ok(random_gif)
}