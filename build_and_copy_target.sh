#!/bin/bash

set -e

IMAGE_NAME="anchor-build-image"
CONTAINER_NAME="temp-anchor-artifacts"
OUTPUT_DIR="./target"

echo "--- Building Docker image: ${IMAGE_NAME} ---"
docker build -t "${IMAGE_NAME}" .

echo "--- Creating temporary container: ${CONTAINER_NAME} ---"

CONTAINER_ID=$(docker create --name "${CONTAINER_NAME}" "${IMAGE_NAME}")

trap 'echo "--- Removing temporary container: ${CONTAINER_NAME} ---"; docker rm -f "${CONTAINER_NAME}" > /dev/null' EXIT

echo "--- Copying artifacts from /app/target in container to ${OUTPUT_DIR} on host ---"

if [ -d "${OUTPUT_DIR}" ]; then
    echo "Removing existing host directory: ${OUTPUT_DIR}"
    rm -rf "${OUTPUT_DIR}"
fi

docker cp "${CONTAINER_NAME}:/app/target" "${OUTPUT_DIR}"

echo "--- Successfully copied artifacts to ${OUTPUT_DIR} ---"

# Exit 0 will remove the container 

exit 0 
