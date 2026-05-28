FROM alpine:3.20

RUN apk add --no-cache openssh-server bash \
 && ssh-keygen -A \
 && adduser -D -s /bin/bash deploy \
 && mkdir -p /home/deploy/.ssh /var/lib/tesoro/downloads \
 && chown -R deploy:deploy /home/deploy /var/lib/tesoro/downloads \
 && sed -i 's/^#\?PermitRootLogin.*/PermitRootLogin no/' /etc/ssh/sshd_config \
 && sed -i 's/^#\?PasswordAuthentication.*/PasswordAuthentication no/' /etc/ssh/sshd_config \
 && sed -i 's/^#\?PubkeyAuthentication.*/PubkeyAuthentication yes/' /etc/ssh/sshd_config

COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

EXPOSE 22
CMD ["/entrypoint.sh"]
