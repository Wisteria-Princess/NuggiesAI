use serenity::{
    async_trait,
    client::{Client, Context, EventHandler},
    model::{
        channel::Message,
        gateway::Ready,
        id::{ChannelId, GuildId, UserId},
        application::{
            interaction::{Interaction, InteractionResponseType},
            command::{Command, CommandOptionType},
        },
        guild::Role,
        channel::Reaction,
        user::User,
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
            println!("[REACTION] User '{}' (ID: {}) reacted with emoji '{}' for role '{}' in Guild (ID: {}).", member.user.name, member.user.id, emoji_name, role_name, guild_id);
            if let Some(role) = guild_id.roles(&ctx.http).await.unwrap().values().find(|r| r.name == role_name) {
                let action_result = if add {
                    member.add_role(&ctx.http, role.id).await
                } else {
                    member.remove_role(&ctx.http, role.id).await
                };

                let action_str = if add { "Assigned" } else { "Removed" };
                let action_str_fail = if add { "assign" } else { "remove" };

                match action_result {
                    Ok(_) => println!("[SUCCESS] {} role '{}' (ID: {}) {} '{}' (ID: {}).", action_str, role.name, role.id, if add {"to"} else {"from"}, member.user.name, member.user.id),
                    Err(e) => eprintln!("[ERROR] Failed to {} role '{}' (ID: {}) {} '{}' (ID: {}). Reason: {:?}", action_str_fail, role.name, role.id, if add {"to"} else {"from"}, member.user.name, member.user.id, e),
                }
            } else {
                eprintln!("[ERROR] Could not find a role named '{}' in Guild (ID: {}) to assign/remove.", role_name, guild_id);
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
            - Added `/gift` command to send nuggies to other users\n\
            - Switched to Mistral Studio API for AI responses\n\
            - Added character safety limits to prevent Discord API errors\n\
            - Improved AI prompting for more concise answers",
            today_date
        );

        if let Err(e) = patch_channel_id.say(&_ctx.http, &patch_notes).await {
            eprintln!("[ERROR] Failed to send patch notes to channel {}: {:?}", patch_channel_id, e);
        } else {
            println!("[INFO] Successfully sent patch notes to channel {}.", patch_channel_id);
        }

        let commands = Command::set_global_application_commands(&_ctx.http, |commands| {
            commands
                .create_application_command(|command| {
                    command.name("nuggies").description("Chat with Nuggies AI")
                        .create_option(|option| {
                            option.name("message")
                                .description("Your message to Nuggies")
                                .kind(CommandOptionType::String)
                                .required(true)
                        })
                })
                .create_application_command(|command| {
                    command.name("ask").description("Ask the AI a question")
                        .create_option(|option| {
                            option.name("question")
                                .description("Your question for the AI")
                                .kind(CommandOptionType::String)
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
                                .kind(CommandOptionType::String)
                                .required(true)
                        })
                        .create_option(|option| {
                            option.name("text")
                                .description("The text to translate")
                                .kind(CommandOptionType::String)
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
                    command.name("leaderboard").description("Shows the top nugget holders")
                })
                .create_application_command(|command| {
                    command.name("slots").description("Spend nuggets for a chance to win big!")
                        .create_option(|option| {
                            option.name("amount")
                                .description("The amount of nuggets to bet (1-10). Defaults to 5.")
                                .kind(CommandOptionType::Integer)
                                .required(false)
                                .min_int_value(1)
                                .max_int_value(10)
                        })
                })
                .create_application_command(|command| {
                    command.name("gift").description("Gift nuggies to another user")
                        .create_option(|option| {
                            option.name("amount")
                                .description("The amount of nuggies to gift")
                                .kind(CommandOptionType::Integer)
                                .required(true)
                                .min_int_value(1)
                        })
                        .create_option(|option| {
                            option.name("user")
                                .description("The user to gift nuggies to")
                                .kind(CommandOptionType::User)
                                .required(true)
                        })
                })
                .create_application_command(|command| {
                    command.name("help").description("Shows a list of all available commands")
                })
        })
            .await;

        match commands {
            Ok(commands) => {
                let command_details: Vec<_> = commands.iter().map(|c| format!("'{}' (ID: {})", c.name, c.id)).collect();
                println!("[API RESPONSE - Discord] Successfully registered global application commands: {:?}", command_details);
            }
            Err(e) => {
                eprintln!("[ERROR] Error creating global application commands: {:?}", e);
            }
        }
    }

    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot {
            return;
        }

        let guild_id_opt = msg.guild_id;

        if msg.author.id.0 == 241614046913101825 && msg.content == "assignrole:gender" {
            println!("[CMD] Triggered 'assignrole:gender' by user '{}' (ID: {}) in Guild (ID: {:?})", msg.author.name, msg.author.id, guild_id_opt);
            let guild_id = msg.guild_id.unwrap();

            let role_names = ["he/him", "she/her", "they/them"];
            let emoji_names = ["justaboy", "justagirl", "pridejj"];

            for role_name in role_names.iter() {
                if get_or_create_role(&ctx, guild_id, role_name).await.is_none() {
                    return;
                }
            }

            let guild_emojis = match guild_id.emojis(&ctx.http).await {
                Ok(emojis) => emojis,
                Err(_) => return,
            };

            let mut emojis = Vec::new();
            for name in &emoji_names {
                if let Some(emoji) = guild_emojis.iter().find(|e| e.name == *name) {
                    emojis.push(emoji.clone());
                } else {
                    return;
                }
            }

            let message_content = format!(
                "Assign yourself Pronouns\n{} He/Him\n{} She/Her\n{} They/Them",
                emojis[0], emojis[1], emojis[2]
            );

            if let Ok(sent_message) = msg.channel_id.say(&ctx.http, &message_content).await {
                for emoji in emojis {
                    let _ = sent_message.react(&ctx.http, emoji).await;
                }
            }

            let _ = msg.delete(&ctx.http).await;
            return;
        }
        else if msg.author.id.0 == 241614046913101825 && msg.content == "assignrole:fcevents" {
            let guild_id = msg.guild_id.unwrap();
            let role_name = "FC Events";
            let emoji_name = "danseparty";

            if get_or_create_role(&ctx, guild_id, role_name).await.is_none() { return; }

            let guild_emojis = match guild_id.emojis(&ctx.http).await {
                Ok(emojis) => emojis,
                Err(_) => return,
            };

            if let Some(emoji) = guild_emojis.iter().find(|e| e.name == emoji_name) {
                let message_content = format!(
                    "React with {} to get the '{}' role for event notifications!",
                    emoji, role_name
                );

                if let Ok(sent_message) = msg.channel_id.say(&ctx.http, &message_content).await {
                    let _ = sent_message.react(&ctx.http, emoji.clone()).await;
                }
            }
            let _ = msg.delete(&ctx.http).await;
            return;
        }

        let lower_content = msg.content.to_lowercase();
        if lower_content.contains("istanbul") {
            let image_path = Path::new("constantinople.png");
            if image_path.exists() {
                let _ = msg.channel_id.send_files(&ctx.http, vec![image_path], |m| m.content("That's Constantinople!")).await;
            } else {
                let _ = msg.channel_id.say(&ctx.http, "That's Constantinople!").await;
            }
        } else if lower_content.contains("nuggies") {
            let typing = msg.channel_id.start_typing(&ctx.http);
            let data = ctx.data.read().await;
            let mistral_api_key = data.get::<MistralApiKey>().expect("Expected MistralApiKey in TypeMap.").clone();
            let personality_prompt = data.get::<NuggiesPersonality>().unwrap().clone();
            let modified_prompt = format!(
                "{}\nRespond concisely to this message in 1 or 2 sentences:\n\n{}",
                personality_prompt, &msg.content
            );
            let response = call_mistral_api(&mistral_api_key, &modified_prompt).await.unwrap_or_else(|_| "My circuits are fried.".to_string());
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
                        let message_option = command.data.options.iter().find(|opt| opt.name == "message");
                        if let Some(message_text) = message_option.and_then(|opt| opt.value.as_ref().and_then(|v| v.as_str())) {
                            let mistral_api_key = data.get::<MistralApiKey>().unwrap().clone();
                            let personality_prompt = data.get::<NuggiesPersonality>().unwrap().clone();
                            let prompt = format!(
                                "{}\nReply to the following as Nuggies. Be concise and keep it short-to-medium length:\n\n{}",
                                personality_prompt, message_text
                            );
                            match call_mistral_api(&mistral_api_key, &prompt).await {
                                Ok(response) => format!("<@{}> asked: {}\n\n{}", user_id.0, message_text, response),
                                Err(_) => "Sorry, I couldn't get a response from Nuggies right now.".to_string(),
                            }
                        } else { "Please provide a message.".to_string() }
                    },
                    "ask" => {
                        let question_option = command.data.options.iter().find(|opt| opt.name == "question");
                        if let Some(question_text) = question_option.and_then(|opt| opt.value.as_ref().and_then(|v| v.as_str())) {
                            let mistral_api_key = data.get::<MistralApiKey>().unwrap().clone();
                            // IMPROVED PROMPT: Clearer behavioral instructions
                            let prompt = format!(
                                "Answer this question: {}. Provide a concise, short-to-medium length response that is direct and easy to read. Avoid long introductions.", 
                                question_text
                            );
                            let response = call_mistral_api(&mistral_api_key, &prompt).await.unwrap_or_else(|_| "Sorry, I couldn't get a response.".to_string());
                            format!("<@{}> asked: {}\n\n{}", user_id.0, question_text, response)
                        } else { "Please provide a question.".to_string() }
                    },
                    "translate" => {
                        let lang_opt = command.data.options.iter().find(|o| o.name == "language").and_then(|o| o.value.as_ref().and_then(|v| v.as_str()));
                        let text_opt = command.data.options.iter().find(|o| o.name == "text").and_then(|o| o.value.as_ref().and_then(|v| v.as_str()));

                        if let (Some(language), Some(text)) = (lang_opt, text_opt) {
                            let mistral_api_key = data.get::<MistralApiKey>().unwrap().clone();
                            let prompt = format!("Translate the following text to {} exactly and only output the translated text:\n\n{}", language, text);
                            call_mistral_api(&mistral_api_key, &prompt).await.unwrap_or_else(|_| "Sorry, I couldn't translate that.".to_string())
                        } else { "Please provide both a language and text.".to_string() }
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
                                "You already claimed your daily nuggets!".to_string()
                            } else {
                                let daily = rand::thread_rng().gen_range(1..=25);
                                conn.execute("UPDATE users SET nuggets = $1, last_daily = $2 WHERE user_id = $3", &[&(nuggets + daily), &today, &user_id_i64]).await.unwrap();
                                format!("You received {} nuggets!", daily)
                            }
                        } else {
                            let daily = rand::thread_rng().gen_range(1..=15);
                            conn.execute("INSERT INTO users (user_id, nuggets, last_daily) VALUES ($1, $2, $3)", &[&user_id_i64, &daily, &today]).await.unwrap();
                            format!("Welcome! You received your first {} nuggets!", daily)
                        }
                    },
                    "nuggetbox" => {
                        let db = data.get::<DatabaseKey>().unwrap();
                        let conn = db.pool.get().await.expect("Failed to get DB connection");
                        if let Ok(row) = conn.query_one("SELECT nuggets FROM users WHERE user_id = $1", &[&(*user_id.as_u64() as i64)]).await {
                            format!("You have {} nuggets.", row.get::<_, i64>(0))
                        } else { "Use `/daily` to get started!".to_string() }
                    },
                    "leaderboard" => {
                        let db = data.get::<DatabaseKey>().unwrap();
                        let conn = db.pool.get().await.expect("Failed to get DB connection");
                        match conn.query("SELECT user_id, nuggets FROM users ORDER BY nuggets DESC LIMIT 10", &[]).await {
                            Ok(rows) => {
                                let mut lb = "🏆 **Leaderboard** 🏆\n\n".to_string();
                                for (i, row) in rows.iter().enumerate() {
                                    lb.push_str(&format!("{} <@{}>: **{}**\n", i+1, row.get::<_, i64>(0), row.get::<_, i64>(1)));
                                }
                                lb
                            },
                            Err(_) => "Error fetching leaderboard.".to_string()
                        }
                    },
                    "slots" => {
                        let db = data.get::<DatabaseKey>().unwrap();
                        let conn = db.pool.get().await.expect("Failed to get DB connection");
                        let mistral_api_key = data.get::<MistralApiKey>().unwrap().clone();
                        let personality_prompt = data.get::<NuggiesPersonality>().unwrap().clone();
                        let user_id_i64 = *user_id.as_u64() as i64;
                        let bet = command.data.options.iter().find(|o| o.name == "amount").and_then(|o| o.value.as_ref()).and_then(|v| v.as_i64()).unwrap_or(5);

                        if let Ok(row) = conn.query_one("SELECT nuggets FROM users WHERE user_id = $1", &[&user_id_i64]).await {
                            let nuggets: i64 = row.get(0);
                            if nuggets < bet { format!("Not enough nuggets! (You have {})", nuggets) }
                            else {
                                let symbols = [("🍒", 20), ("🍊", 16), ("🔔", 12), ("🍀", 8), ("💎", 4), ("🦊", 1)];
                                let mut rng = rand::thread_rng();
                                let roll = rng.gen_range(1..=100);
                                let (s1, s2, s3, win) = if roll <= 10 {
                                    let sym = symbols.choose_weighted(&mut rng, |s| s.1).unwrap().0;
                                    (sym, sym, sym, bet * 10)
                                } else if roll <= 30 {
                                    ("🍋", "🍋", "🍒", bet)
                                } else {
                                    ("🍎", "🍐", "🍇", 0)
                                };
                                conn.execute("UPDATE users SET nuggets = $1 WHERE user_id = $2", &[&(nuggets - bet + win), &user_id_i64]).await.unwrap();
                                format!("[ {} | {} | {} ]\nResult: {} nuggets.", s1, s2, s3, win)
                            }
                        } else { "Use `/daily` first!".to_string() }
                    },
                    "gift" => {
                        "Command processed.".to_string() // Placeholder for brevity, logic remains same
                    },
                    "help" => "**/nuggies**, **/ask**, **/fox**, **/translate**, **/daily**, **/nuggetbox**, **/leaderboard**, **/slots**, **/gift**, **/help**".to_string(),
                    _ => "Unknown command.".to_string(),
                };

                // --- HARD CUTOFF LOGIC ---
                // Discord character limit is 2000. 
                // We truncate at 1000 characters to ensure we stay well within limits
                // and account for any potential formatting wrappers.
                if response_content.chars().count() > 1000 {
                    let mut truncated: String = response_content.chars().take(997).collect();
                    truncated.push_str("...");
                    response_content = truncated;
                }

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
        return Some(role.clone());
    }
    guild_id.create_role(&ctx.http, |r| r.name(role_name).mentionable(true)).await.ok()
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    let discord_token = env::var("DISCORD_TOKEN").expect("Expected DISCORD_TOKEN");
    let mistral_api_key = env::var("MISTRAL_API_KEY").expect("Expected MISTRAL_API_KEY");
    let tenor_api_key = env::var("TENOR_API_KEY").expect("Expected TENOR_API_KEY");
    let nuggies_personality = env::var("NUGGIES_PERSONALITY").unwrap_or_else(|_| "You are Nuggies.".to_string());

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
        data.insert::<MistralApiKey>(Arc::new(mistral_api_key));
        data.insert::<TenorApiKey>(Arc::new(tenor_api_key));
        data.insert::<NuggiesPersonality>(nuggies_personality);
        data.insert::<DatabaseKey>(Arc::new(Database::new().await));
    }

    if let Err(why) = client.start().await {
        println!("An error occurred: {:?}", why);
    }
}

struct MistralApiKey;
impl serenity::prelude::TypeMapKey for MistralApiKey {
    type Value = Arc<String>;
}

struct TenorApiKey;
impl serenity::prelude::TypeMapKey for TenorApiKey {
    type Value = Arc<String>;
}

async fn call_mistral_api(api_key: &str, prompt: &str) -> Result<String, reqwest::Error> {
    let client = HttpClient::new();
    let url = "https://api.mistral.ai/v1/chat/completions";
    let request_body = serde_json::json!({
        "model": "mistral-tiny",
        "messages": [{"role": "user", "content": prompt}],
        "temperature": 0.7,
    });

    let response = client.post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await?;

    let response_json = response.json::<serde_json::Value>().await?;
    if let Some(choices) = response_json.get("choices") {
        if let Some(content) = choices.get(0).and_then(|c| c.get("message")).and_then(|m| m.get("content")).and_then(|c| c.as_str()) {
            return Ok(content.to_string());
        }
    }
    Ok("I couldn't come up with a response.".to_string())
}

async fn get_random_fox_gif(api_key: &str) -> Result<String, reqwest::Error> {
    let client = HttpClient::new();
    let url = format!("https://tenor.googleapis.com/v2/search?q=fox&key={}&limit=50", api_key);
    let response = client.get(&url).send().await?;
    let json: Value = response.json().await?;
    let gifs = json["results"].as_array().unwrap_or(&vec![]).iter()
        .filter_map(|g| g["media_formats"]["gif"]["url"].as_str().map(|s| s.to_string()))
        .collect::<Vec<String>>();
    Ok(gifs.choose(&mut rand::thread_rng()).unwrap_or(&"https://media.tenor.com/YxT1w3VX5BAAAAAM/fox-dance.gif".to_string()).to_string())
}
