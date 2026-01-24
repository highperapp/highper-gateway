{{/*
Expand the name of the chart.
*/}}
{{- define "highper-gateway.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "highper-gateway.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Create chart name and version as used by the chart label.
*/}}
{{- define "highper-gateway.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "highper-gateway.labels" -}}
helm.sh/chart: {{ include "highper-gateway.chart" . }}
{{ include "highper-gateway.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
app.kubernetes.io/use-case: {{ .Values.useCase | quote }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "highper-gateway.selectorLabels" -}}
app.kubernetes.io/name: {{ include "highper-gateway.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Create the name of the service account to use
*/}}
{{- define "highper-gateway.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "highper-gateway.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
Get service ports for the configured use case
*/}}
{{- define "highper-gateway.servicePorts" -}}
{{- $useCase := .Values.useCase | quote }}
{{- $ports := index .Values.service.ports .Values.useCase }}
{{- if $ports }}
{{- toYaml $ports }}
{{- else }}
{{- toYaml (index .Values.service.ports "02") }}
{{- end }}
{{- end }}
