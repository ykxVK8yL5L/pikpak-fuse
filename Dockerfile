FROM alpine:latest
ARG TARGETARCH
ARG TARGETVARIANT
RUN apk --no-cache add ca-certificates tini fuse3
RUN apk add tzdata && \
	cp /usr/share/zoneinfo/Asia/Shanghai /etc/localtime && \
	echo "Asia/Shanghai" > /etc/timezone && \
	apk del tzdata

RUN mkdir -p /etc/PikpakDrive-fuse /mnt/PikpakDrive
WORKDIR /root/
ADD PikpakDrive-fuse-$TARGETARCH$TARGETVARIANT /usr/bin/PikpakDrive-fuse

ENTRYPOINT ["/sbin/tini", "--"]
CMD ["/usr/bin/PikpakDrive-fuse", "--workdir", "/etc/PikpakDrive-fuse", "/mnt/PikpakDrive"]
