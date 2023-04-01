# Install websocat: cargo install --features=ssl websocat

ADDR="http://localhost:8787"
ORIGIN="http://localhost:3000"

# hint
echo "GET /hint"
curl -H "Origin: $ORIGIN" -X GET $ADDR/hint -v

# main (You need to set secret key to test key first)
REQUEST="{\"question\":\"Hi, Who are you?\", \"captcha_token\":\"dummy\"}"
echo
echo "WS /chat -> $REQUEST"
echo "$REQUEST" | websocat ws://localhost:8787/chat