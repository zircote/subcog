FROM grafana/grafana:latest

COPY --chown=grafana:grafana docker/grafana/provisioning /etc/grafana/provisioning
COPY --chown=grafana:grafana docker/dashboards /var/lib/grafana/dashboards

# Run as non-root user (grafana user from base image)
USER grafana
