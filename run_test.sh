#!/bin/bash

set -e

function size_to_tasks {
	case "$1" in
	S) echo 5
		;;
	M) echo 10
		;;
	L) echo 20
		;;
	XL) echo 40
		;;
	XXL) echo 80
		;;
	esac
}

if [ -z "$1" ]; then
  echo "Usage: $0 cpu|db"
  echo "  cpu: cpu bound test"
  echo "  db: io bound test"
  exit 1
fi

pushd rmeter
cargo build --release
popd

for s in S M L XL XXL; do
	echo "== $s"
	./rmeter/target/release/rmeter --api $1 --tasks $(size_to_tasks $s) --loop 20
done
