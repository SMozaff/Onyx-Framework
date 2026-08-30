{{- define "onyx-api-relay.name" -}}onyx-api-relay{{- end -}}
{{- define "onyx-api-relay.fullname" -}}{{ .Release.Name | default "onyx-api-relay" }}{{- end -}}
{{- define "onyx-api-relay.labels" -}}
app.kubernetes.io/name: {{ include "onyx-api-relay.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end -}}
