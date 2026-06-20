# Dockerfile syntax fixture
# Multi-line comment
# is just multiple lines

FROM python:3.12-slim AS builder
ARG DEBIAN_FRONTEND=noninteractive
LABEL maintainer="dev@example.com"
LABEL version="1.0"

ENV APP_HOME=/app
ENV PYTHONDONTWRITEBYTECODE=1
ENV POETRY_VERSION=1.8.0

WORKDIR ${APP_HOME}

RUN apt-get update && \
    apt-get install -y --no-install-recommends gcc && \
    rm -rf /var/lib/apt/lists/*

COPY pyproject.toml poetry.lock ./
RUN pip install "poetry==$POETRY_VERSION"
RUN poetry install --no-interaction --no-ansi

COPY . .

EXPOSE 8000/tcp

VOLUME ["/data"]

HEALTHCHECK --interval=30s --timeout=3s \
  CMD curl -f http://localhost:8000/ || exit 1

USER appuser

ENTRYPOINT ["poetry", "run"]
CMD ["uvicorn", "app.main:app", "--host", "0.0.0.0"]

FROM python:3.12-slim
COPY --from=builder /app /app
WORKDIR /app
CMD ["python", "-m", "app"]
