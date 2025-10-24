cargo run -- listen --port 8000 --name Node1


cargo run -- connect --addr 127.0.0.1 --port 8000 --name Node2


curl -s -X POST http://127.0.0.1:9000/api/tx \
  -H 'content-type: application/json' \
  -d '{"message":"hello-from-curl"}' | jq


curl -s http://127.0.0.1:9000/api/debug | jq
curl -s http://127.0.0.1:9000/api/mempool | jq
curl -s http://127.0.0.1:9000/api/blocks | jq
