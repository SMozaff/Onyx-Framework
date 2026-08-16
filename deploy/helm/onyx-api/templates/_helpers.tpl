{{- define "onyx-api.name" -}}onyx-api{{- end -}}
{{- define "onyx-api.fullname" -}}{{ .Release.Name | default "onyx-api" }}{{- end -}}
{{- define "onyx-api.labels" -}}
app.kubernetes.io/name: {{ include "onyx-api.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end -}}
