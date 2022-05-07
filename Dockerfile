FROM alpine:latest
ARG TARGETARCH
ARG TARGETVARIANT
RUN apk --no-cache add ca-certificates tini fuse3
RUN apk add tzdata && \
	cp /usr/share/zoneinfo/Asia/Shanghai /etc/localtime && \
	echo "Asia/Shanghai" > /etc/timezone && \
	apk del tzdata

RUN mkdir -p /etc/Pikpak-fuse /mnt/PikpakDrive
WORKDIR /root/
ADD Pikpak-fuse-$TARGETARCH$TARGETVARIANT /usr/bin/Pikpak-fuse

ENTRYPOINT ["/sbin/tini", "--"]
CMD ["/usr/bin/Pikpak-fuse", "--workdir", "/etc/Pikpak-fuse", "/mnt/PikpakDrive"]
