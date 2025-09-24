# Nuggies AI Discord Bot

Nuggies is a multi-purpose, AI-powered Discord bot built in Rust. It features an interactive AI chat with a unique personality, a server currency system, reaction roles, GIF commands, and more. The bot is designed as a Personal Project for my own Community.

## Features

- **AI Chat**: Chat directly with Nuggies using the `/nuggies` command or by mentioning its name in a message. The AI is powered by Google's Gemini 1.5 Flash model and has a customizable personality.
- **Currency System**: A simple and fun server economy. Users can claim a daily amount of "nuggets" with `/daily` and check their balance with `/nuggetbox`. All data is stored in a local SQLite database.
- **Reaction Roles**: Allow users to self-assign roles by reacting to a specific message. This is fully configurable for different roles and emojis.
- **GIF Commands**: Fetch random GIFs using simple commands (e.g., `/fox`).
- **Utility Commands**: Includes a `/translate` command to translate text into different languages.
- **Automatic Responses**: The bot can be configured to automatically respond to certain keywords in messages (e.g., "istanbul") for fun.

## Commands

### Slash Commands

- `/nuggies <message>`: Chat with the Nuggies AI.
- `/ask <question>`: Ask the AI a general question without the personality overlay.
- `/translate <language> <text>`: Translates the given text into the specified language.
- `/fox`: Fetches a random fox GIF from Tenor.
- `/daily`: Claim between 1 and 10 "nuggets" once per day (resets at 00:00 GMT+2).
- `/nuggetbox`: Check your current balance of nuggets.

### Message Triggers

- `nuggies`: Mentioning "nuggies" in a message will trigger a response from the AI.
- `istanbul`: Mentioning "istanbul" will make the bot reply with a "Constantinople" meme.

### Admin-Only Message Commands

These commands must be sent by a whitelisted user ID and are used to set up the reaction role messages. The original command message is deleted after execution.

- `assignrole:gender`: Creates a message for users to self-assign pronoun roles (he/him, she/her, they/them).
- `assignrole:fcevents`: Creates a message for users to get an "FC Events" role for notifications.

## Setup and Installation

Follow these steps to run your own instance of the Nuggies AI bot.

### Prerequisites

- [Rust and Cargo](https://www.rust-lang.org/tools/install) installed on your system.

### 1. Clone the Repository

```sh
git clone https://your-repository-url/discord-gemini-bot.git
cd discord-gemini-bot
```

### 2. Configure Environment Variables

Create a file named `.env` in the root of the project directory. This file will store your secret keys and tokens.

```env
DISCORD_TOKEN=YOUR_DISCORD_BOT_TOKEN
GEMINI_API_KEY=YOUR_GOOGLE_GEMINI_API_KEY
TENOR_API_KEY=YOUR_TENOR_API_KEY
```

**Where to get the keys:**
- `DISCORD_TOKEN`: Create an application on the [Discord Developer Portal](https://discord.com/developers/applications). Go to the "Bot" tab and copy the token.
- `GEMINI_API_KEY`: Get your API key from [Google AI Studio](https://aistudio.google.com/app/apikey).
- `TENOR_API_KEY`: Get your API key from the [Tenor GIF API](https://tenor.com/gifapi/documentation).

### 3. Build and Run the Bot

You can run the bot in debug mode or build it for release for better performance.

**To run in debug mode:**
```sh
cargo run
```

**To build and run in release mode:**```sh
# First, build the optimized executable
cargo build --release

# Then, run the executable
./target/release/discord-gemini-bot
```

The bot should now be online and connected to your server!

## Technologies Used

- **Language**: [Rust](https://www.rust-lang.org/)
- **Discord API Wrapper**: [Serenity](https://github.com/serenity-rs/serenity)
- **Asynchronous Runtime**: [Tokio](https://tokio.rs/)
- **HTTP Client**: [Reqwest](https://docs.rs/reqwest/latest/reqwest/)
- **Database**: [Rusqlite](https://github.com/rusqlite/rusqlite) (SQLite)
- **AI Model**: Google Gemini API
- **GIFs**: Tenor API
