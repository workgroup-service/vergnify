# @author Cursor

FROM golang:1.25-alpine AS build
WORKDIR /src
COPY go.mod go.sum ./
RUN go mod download
COPY . .
RUN CGO_ENABLED=0 go build -o /out/admin .

FROM alpine:3.20
RUN apk add --no-cache ca-certificates
WORKDIR /app
COPY --from=build /out/admin /app/admin
COPY templates /app/templates
EXPOSE 8080
ENTRYPOINT ["/app/admin"]
