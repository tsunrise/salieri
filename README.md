# Salieri System

## Overview
This repository contains the source code for the Salieri System, a component of my personal website located at <https://tomshen.io>.

The Salieri System is an API gateway developed in Rust that seamlessly integrates OpenAI's Chat API into my website. The gateway is deployed using Cloudflare Workers, providing fast and secure access to the service.

## Prerequisites
To build and deploy the Salieri System, you need to have Rust installed on your system. If you don't have Rust, you can install it by following the instructions at <https://rustup.rs/>.

## Configuration
To successfully build this service, a `config.toml` file is required. This file specifies various configuration options, including the welcome message, hint questions, and the model used for the OpenAI Chat API. An example `config.toml` file is provided below:

```toml
welcome = "<Your welcome message>"
questions = [
    "Who are you?",
    # insert your hint questions here
]

[prompt]
messages = [
    { role = "system", content = "You are a digital copy of Tom." },
    { role = "user", content = "Act as Tom... <your instruction>" },
    { role = "assistant", content = "Understood. I will act as Tom and answer very concisely." },
]
model = "gpt-3.5-turbo"
max_tokens = 128
```

## Build and Deployment

After configuring the `config.toml` file, you can build and deploy the Salieri System using the following steps:

1. Open your terminal and navigate to the root directory of the Salieri System repository.

2. Use Wrangler CLI to publish the service to Cloudflare Workers. If you haven't installed Wrangler, you can do so using npm. Run the following command to publish the service:

```bash
npx wrangler publish
```

The `wrangler publish` command will build the Rust code, package it as a Cloudflare Worker, and deploy it to your Cloudflare account. Once the deployment is successful, you'll receive a confirmation message with the URL of your deployed service.

## License
This project is licensed under the [MIT License](LICENSE).
