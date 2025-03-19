#!/bin/bash
set -e

# Build and package 
cargo build --release --target x86_64-unknown-linux-musl
mkdir -p .aws-sam/build/bootstrap/
cp ./target/x86_64-unknown-linux-musl/release/lockbox-box-service .aws-sam/build/bootstrap/bootstrap

# Deploy using SAM CLI
sam deploy --template-file template.yaml --stack-name lockbox-box-service --capabilities CAPABILITY_IAM
