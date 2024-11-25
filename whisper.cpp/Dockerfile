FROM whisper.cpp:self-main-cuda

RUN apt-get update && apt-get install tini

COPY docker-entrypoint.sh /app
RUN chmod +x /app/docker-entrypoint.sh

ENTRYPOINT ["tini", "--", "/app/docker-entrypoint.sh"]
