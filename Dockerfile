# Use the official Rust image as a base
FROM rust:latest as builder

# Set the working directory
WORKDIR /usr/src/app

# Copy your project files into the container
COPY . .

# Build your application in release mode
RUN cargo build --release

# --- Final Stage ---
# Use a smaller, more secure base image for the final container
FROM debian:buster-slim

# Install the OpenSSL runtime library, which is required by reqwest for HTTPS.
# We also clean up the apt cache to keep the image small.
RUN apt-get update && apt-get install -y libssl3 && rm -rf /var/lib/apt/lists/*

# Copy the compiled binary from the builder stage
COPY --from=builder /usr/src/app/target/release/discord-gemini-bot /usr/local/bin/discord-gemini-bot

# Copy any assets your bot needs
COPY constantinople.png .

# Set the command to run your bot when the container starts
CMD ["/usr/local/bin/discord-gemini-bot"]
