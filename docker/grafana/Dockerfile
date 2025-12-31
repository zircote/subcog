FROM grafana/grafana:10.4.3

COPY --chown=grafana:grafana docker/grafana/provisioning /etc/grafana/provisioning
COPY --chown=grafana:grafana docker/dashboards /var/lib/grafana/dashboards
