FROM alpine:latest
RUN sed -i 's#dl-cdn.alpinelinux.org#mirrors.aliyun.com#g' /etc/apk/repositories \
 && apk add --no-cache supervisor ca-certificates tzdata \
 && rm -rf /var/cache/apk/*
COPY supervisord.conf /etc/supervisord.conf
EXPOSE 58881
CMD ["/usr/bin/supervisord","-c","/etc/supervisord.conf"]
