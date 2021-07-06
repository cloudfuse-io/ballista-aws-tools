FROM ubuntu

RUN apt-get update && \
    apt-get install -y git build-essential

RUN git clone https://github.com/databricks/tpch-dbgen.git && \
    cd tpch-dbgen && \
    make

WORKDIR /tpch-dbgen
COPY ./docker/tpch/entrypoint.sh /tpch-dbgen/entrypoint.sh

VOLUME /data

ENTRYPOINT [ "bash", "./entrypoint.sh" ]
