# Nuggies AI Discord Bot

Welcome to the repository for Nuggies AI, a custom-built Discord bot created as a personal project for a private community. This bot integrates a variety of fun and interactive features to enhance the server experience.

The official repository can be found at: [NuggiesAI](https://github.com/Wisteria-Princess/NuggiesAI)

## Features

- **AI Chat**: Chat directly with Nuggies using the `/nuggies` command or by mentioning its name in a message. The AI is powered by Google's Gemini Pro model and has a unique, customizable personality (currently a slightly unhinged, sarcastic, Norse pagan goth).
- **Currency System**: A simple and fun server economy centered around "nuggets."
  - `/daily`: Claim a random amount of nuggets once per day.
  - `/nuggetbox`: Check your current balance.
  - `/slots`: Spend 5 nuggets to play the slots for a chance to win big, complete with a witty remark from Nuggies.
- **Reaction Roles**: Allows users to self-assign roles by reacting to specific messages, set up by a server admin.
- **Utility Commands**: Includes a `/fox` command for random GIFs and a `/translate` command for translating text.
- **Automatic Responses**: The bot is configured to automatically respond to certain keywords in messages for extra flavor.

## Commands

### Slash Commands

- `/nuggies <message>`: Chat with the Nuggies AI.
- `/ask <question>`: Ask the AI a general question without the personality overlay.
- `/translate <language> <text>`: Translates the given text into the specified language.
- `/fox`: Fetches a random fox GIF from Tenor.
- `/daily`: Claim between 1 and 15 "nuggets" once per day.
- `/nuggetbox`: Check your current balance of nuggets.
- `/slots`: Spend 5 nuggets for a chance to win a variable amount of nuggets.

## Technologies Used

- **Language**: [Rust](https://www.rust-lang.org/)
- **Discord API Wrapper**: [Serenity](https://github.com/serenity-rs/serenity)
- **Asynchronous Runtime**: [Tokio](https://tokio.rs/)
- **HTTP Client**: [Reqwest](https://docs.rs/reqwest/latest/reqwest/)
- **Database**: [Rusqlite](https://github.com/rusqlite/rusqlite) (SQLite)
- **AI Model**: Google Gemini API
- **GIFs**: Tenor API