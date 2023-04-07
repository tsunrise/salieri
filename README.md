# Salieri System

## Overview

The Salieri System is an API gateway that integrates OpenAI's Chat API into my personal website <https://tomshen.io>, deployed using Cloudflare Workers.

**Warning:** Please note that the Salieri System is specifically designed for my personal use and may contain custom configurations tailored to my website. As a result, you may find it nontrivial to integrate into your own projects. 

## Prerequisites
To build and deploy the Salieri System, you need to have Rust installed on your system. If you don't have Rust, you can install it by following the instructions at <https://rustup.rs/>.

## Configuration

You need to setup the following environment variables: 
- `TURNSTILE_SECRET_KEY`: The secret key for Cloudflare Turnstile. 
- `OPENAI_API_KEY`: The API key for OpenAI's Chat API.

You can use the following command to set the environment variables:

```bash
wrangler secret put <variable_name>
```

You will also need to set your `config` entry inside a Cloudflare Workers KV store. A template `config` entry is as follows
```json
{
    "prompt": {
        "model": "gpt-3.5-turbo",
        "messages": [
            {
                "role": "system",
                "content": "..."
            },
            {
                "role": "user",
                "content": "..."
            },
            {
                "role": "assistant",
                "content": "..."
            }
        ],
        "max_tokens": 200
    },
    "questions": [
        "...",
        "...",
        "..."
    ],
    "welcome": "...",
    "announcement": null
}
```

## Build and Deployment

You can build and deploy the Salieri System using the following steps:

1. Open your terminal and navigate to the root directory of the Salieri System repository.

2. Use Wrangler CLI to publish the service to Cloudflare Workers. If you haven't installed Wrangler, you can do so using npm. Run the following command to publish the service:

```bash
npx wrangler publish
```

The `wrangler publish` command will build the Rust code, package it as a Cloudflare Worker, and deploy it to your Cloudflare account. Once the deployment is successful, you'll receive a confirmation message with the URL of your deployed service.

## License
This project is licensed under the [MIT License](LICENSE).
