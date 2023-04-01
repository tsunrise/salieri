# Install websocat: cargo install --features=ssl websocat

ADDR="http://localhost:8787"
ORIGIN="http://localhost:3001"

# hint
echo "GET /hint"
curl -H "Origin: $ORIGIN" -X GET $ADDR/hint -v

# main
echo 
echo "WS /chat -> {\"question\":\"Hi, Who are you?\"}"
echo "{\"question\":\"Hi, Who are you?\"}" | websocat ws://localhost:8787/chat