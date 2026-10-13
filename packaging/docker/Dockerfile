# CloudMount 边缘网关多架构镜像。
#
# 构建（buildx 多架构，musl 静态产物）：
#   docker buildx build --platform linux/amd64,linux/arm64 \
#     -f packaging/docker/Dockerfile -t tintac/cloudmount:latest .
#
# 运行：
#   docker run -d -p 8384:8384 \
#     -e CLOUDMOUNT_APP_ID=xxx \
#     tintac/cloudmount:latest --serve --listen 0.0.0.0:8384 --auth user:pass
#
# 登录（交互式，一次性）：
#   docker run -it --rm -e CLOUDMOUNT_APP_ID=xxx tintac/cloudmount:latest cloudmount login

# ---- 静态产物层（buildx 会按目标架构选择对应阶段）----
FROM scratch AS binary-amd64
COPY target/x86_64-unknown-linux-musl/release/cloudmountd /out/cloudmountd
COPY target/x86_64-unknown-linux-musl/release/cloudmount /out/cloudmount

FROM scratch AS binary-arm64
COPY target/aarch64-unknown-linux-musl/release/cloudmountd /out/cloudmountd
COPY target/aarch64-unknown-linux-musl/release/cloudmount /out/cloudmount

# ---- 运行层 ----
FROM alpine:3.20
# TARGETARCH：amd64 / arm64（buildx 注入）——选择对应产物层
ARG TARGETARCH
COPY --from=binary-${TARGETARCH} /out/cloudmountd /usr/local/bin/cloudmountd
COPY --from=binary-${TARGETARCH} /out/cloudmount /usr/local/bin/cloudmount
COPY web-ui/dist /opt/cloudmount/web-ui

# ca-certificates：HTTPS 通道；tzdata：时间戳
RUN apk add --no-cache ca-certificates tzdata && \
    adduser -D -H -s /sbin/nologin cloudmount && \
    mkdir -p /var/lib/cloudmount && chown cloudmount /var/lib/cloudmount

USER cloudmount
ENV CLOUDMOUNT_DATA_DIR=/var/lib/cloudmount
EXPOSE 8384
VOLUME /var/lib/cloudmount

ENTRYPOINT ["/usr/local/bin/cloudmountd"]
CMD ["--serve", "--listen", "0.0.0.0:8384"]
