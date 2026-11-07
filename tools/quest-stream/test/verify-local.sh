#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
export JAVA_HOME="${JAVA_HOME:-/opt/homebrew/opt/openjdk}"
export PATH="$JAVA_HOME/bin:$PATH"
mkdir -p build/testclasses
javac -d build/testclasses src/cz/honza/queststream/LocalServer.java test/LocalServerTest.java
java -cp build/testclasses LocalServerTest
javac -d build/testclasses src/cz/honza/queststream/StreamHub.java test/StreamHubTest.java
java -cp build/testclasses StreamHubTest
node --check assets/viewer.js
./test/verify-mp4.sh
