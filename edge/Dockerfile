# CloudMount 边缘节点镜像
# 多阶段构建：rust 编译 → scratch 运行（静态 musl 二进制 + web-ui 静态文件）
#
# 构建多架构（在 CI 用 buildx）：
#   docker buildx build --platform linux/amd64,linux/arm64,linux/arm/v7 -t cloudmountd .

FROM --platform=$BUILDPLATFORM rust:1-alpine AS build
ARG TARGETPLATFORM
ARG BUILDARCH
# cross-rs 风格目标映射（CI 中由 buildx 目标平台决定）
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY . .
# arm64/armv7 交叉编译在 CI 中改用 cross 工具链；此处默认本机构建
RUN cargo build -p cloudmount-daemon --release

FROM scratch
COPY --from=build /src/target/release/cloudmountd /cloudmountd
COPY --from=build /src/web-ui/dist /web-ui
# 数据/缓存挂载点
VOLUME ["/data"]
ENV CLOUDMOUNT_DATA_DIR=/data \
    CLOUDMOUNT_WEB_UI=/web-ui \
    CLOUDMOUNT_LISTEN=0.0.0.0:8384
EXPOSE 8384
ENTRYPOINT ["/cloudmountd"]
