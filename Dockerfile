FROM alpine:latest
ARG TARGETARCH
ARG TARGETVARIANT
RUN apk --no-cache add ca-certificates tini fuse3
RUN apk add tzdata && \
	cp /usr/share/zoneinfo/Asia/Shanghai /etc/localtime && \
	echo "Asia/Shanghai" > /etc/timezone && \
	apk del tzdata

RUN mkdir -p /etc/pikpak-fuse /mnt/pikpakDrive
WORKDIR /root/
ADD pikpak-fuse-$TARGETARCH$TARGETVARIANT /usr/bin/pikpak-fuse

ENTRYPOINT ["/sbin/tini", "--"]
CMD ["/usr/bin/pikpak-fuse", "--workdir", "/etc/pikpak-fuse", "/mnt/pikpakDrive"]
