source ../solana-trading/run/utils.sh

if [[ "$1" == "--no-kill" ]]; then
    shift  # Discard the first argument, so $1 will now be the new first argument if there was one
else
    kill_process "openbookv2-printer"
fi

# RUST_LOG=info  ensure_running $1 "./target/release/openbookv2-printer -- --market $markets --rpc-url $rpc_url --grpc https://spacemonkey.rpcpool.com --x-token 033e7b48-8f8c-439f-a311-d76416646135" ~/log openbook-trades-printer
# ./target/release/openbookv2-printer -- --market $markets --rpc-url $rpc_url --grpc https://spacemonkey.rpcpool.com --x-token 033e7b48-8f8c-439f-a311-d76416646135
RUST_LOG=info ensure_running $1 "./target/debug/openbookv2-printer" ~/log openbook-trades-printer