FROM  ubuntu:22.04

#  软件源和时区
RUN sed -i 's/archive.ubuntu.com/mirrors.aliyun.com/g' /etc/apt/sources.list && \
    sed -i 's/security.ubuntu.com/mirrors.aliyun.com/g' /etc/apt/sources.list && \
    apt-get update && \
    apt-get install -yq apt-utils tzdata && \
    ln -fs /usr/share/zoneinfo/Asia/Shanghai /etc/localtime && \
    dpkg-reconfigure -f noninteractive tzdata

# 基础软件 这个看情况
RUN  apt install vim -yq

# supervisor
RUN  apt install supervisor -yq && \
    mkdir -p /data/app/log/supervisor/
COPY config/supervisor/supervisord.conf /etc/supervisor/supervisord.conf
COPY config/start.sh /start.sh

RUN apt-get clean -yq

# app
RUN mkdir -p /opt/app
RUN mkdir -p /opt/app/config
RUN mkdir -p /opt/app/logs
COPY tmp/ /opt/app/
COPY config/app/app_start.sh /opt/app/
COPY config/supervisor/conf.d/app.conf /etc/supervisor/conf.d/app.conf

ENV SYNC_BASE=/opt/app/data

CMD ["/start.sh"]
