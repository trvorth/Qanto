# --- Stage 1: Builder ---
# Use the latest stable version of Rust to build our application.
FROM rust:latest as builder

# Set the working directory
WORKDIR /usr/src/qanto

# Copy the entire project context into the builder
COPY . .

# Install dependencies required for the build (like clang and libtorch)
RUN apt-get update && apt-get install -y clang libclang-dev wget unzip

# Download and unzip libtorch (CPU version)
RUN wget https://download.pytorch.org/libtorch/cpu/libtorch-cxx11-abi-shared-with-deps-2.3.1%2Bcpu.zip && \
    unzip libtorch-cxx11-abi-shared-with-deps-2.3.1%2Bcpu.zip && \
    rm libtorch-cxx11-abi-shared-with-deps-2.3.1%2Bcpu.zip
ENV LIBTORCH=/usr/src/qanto/libtorch

# Build the project in release mode with the 'ai' feature
RUN cargo build --release --features ai


# --- Stage 2: Final Image ---
# Use a minimal, secure base image for our final container.
FROM debian:bullseye-slim

# Install only necessary runtime dependencies
RUN apt-get update && apt-get install -y openssl ca-certificates && rm -rf /var/lib/apt/lists/*

# Set the working directory
WORKDIR /usr/local/bin

# Copy the compiled binary from the 'builder' stage
COPY --from=builder /usr/src/qanto/target/release/qanto .

# Make the binary executable
RUN chmod +x qanto

# Set the entrypoint to run the node when the container starts
ENTRYPOINT ["./qanto"]