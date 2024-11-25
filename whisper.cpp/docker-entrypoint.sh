#!/bin/bash

MODEL_KIND=ggml
MODEL=large-v3-turbo

if [ ! -f /app/models_ext/$MODEL_KIND-$MODEL ]; then
    echo "Model not found: $MODEL_KIND-$MODEL"
    echo "Downloading the model..."
    if /app/models/download-$MODEL_KIND-model.sh $MODEL models_ext; then
        echo "Model downloaded successfully"
    else
        echo "Failed to download the model"
        exit 1
    fi
fi

/app/server -m models_ext/ggml-large-v3-turbo.bin --host 0.0.0.0 -pc -pr -l it -debug

