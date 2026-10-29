// @author Cursor

package main

import (
	"context"
	"crypto/rand"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"log"
	"net"
	"net/http"
	"net/smtp"
	"os"
	"strings"
	"time"
)

const codeAlphabet = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789"

func normalizeCode(code string) string {
	var b strings.Builder
	for _, ch := range strings.ToUpper(code) {
		if ch == '-' || ch == ' ' {
			continue
		}
		b.WriteRune(ch)
	}
	return b.String()
}

func newUserCode() (string, error) {
	buf := make([]byte, 8)
	if _, err := rand.Read(buf); err != nil {
		return "", err
	}
	chars := make([]byte, 8)
	for i, value := range buf {
		chars[i] = codeAlphabet[int(value)%len(codeAlphabet)]
	}
	return string(chars), nil
}

func publicURL() string {
	if value := strings.TrimSpace(os.Getenv("PUBLIC_URL")); value != "" {
		return strings.TrimRight(value, "/")
	}
	return "http://127.0.0.1:8080"
}

func sendInvite(email, acceptURL string) {
	host := strings.TrimSpace(os.Getenv("SMTP_HOST"))
	if host == "" {
		log.Printf("invite for %s: %s", email, acceptURL)
		return
	}
	port := os.Getenv("SMTP_PORT")
	if port == "" {
		port = "1025"
	}
	body := fmt.Sprintf("To: %s\r\nSubject: Vergnify invitation\r\n\r\n%s\r\n", email, acceptURL)
	err := smtp.SendMail(net.JoinHostPort(host, port), nil, "vergnify@localhost", []string{email}, []byte(body))
	if err != nil {
		log.Printf("invite mail failed for %s: %v (%s)", email, err, acceptURL)
	}
}

func createInvite(ctx context.Context, db *sql.DB, orgID int64, email string) (string, error) {
	raw, err := randomToken()
	if err != nil {
		return "", err
	}
	_, err = db.ExecContext(ctx, `
		INSERT INTO invitations (org_id, email, token_hash, expires_at)
		VALUES ($1, $2, $3, now() + interval '7 days')
	`, orgID, email, hashToken(raw))
	if err != nil {
		return "", err
	}
	acceptURL := publicURL() + "/invitations/accept?token=" + raw
	sendInvite(email, acceptURL)
	return acceptURL, nil
}

func acceptInvite(ctx context.Context, db *sql.DB, rawToken string) (account, error) {
	var inviteID, orgID int64
	var email string
	var accepted sql.NullTime
	var expires time.Time
	err := db.QueryRowContext(ctx, `
		SELECT id, org_id, email, expires_at, accepted_at
		FROM invitations
		WHERE token_hash = $1
	`, hashToken(rawToken)).Scan(&inviteID, &orgID, &email, &expires, &accepted)
	if err != nil {
		return account{}, err
	}
	if accepted.Valid || time.Now().After(expires) {
		return account{}, sql.ErrNoRows
	}

	tx, err := db.BeginTx(ctx, nil)
	if err != nil {
		return account{}, err
	}
	defer tx.Rollback()

	var user account
	err = tx.QueryRowContext(ctx, `
		SELECT id, org_id, email, role FROM users WHERE org_id = $1 AND email = $2
	`, orgID, email).Scan(&user.ID, &user.OrgID, &user.Email, &user.Role)
	if errors.Is(err, sql.ErrNoRows) {
		err = tx.QueryRowContext(ctx, `
			INSERT INTO users (org_id, email, role)
			VALUES ($1, $2, 'member')
			RETURNING id, org_id, email, role
		`, orgID, email).Scan(&user.ID, &user.OrgID, &user.Email, &user.Role)
	}
	if err != nil {
		return account{}, err
	}
	result, err := tx.ExecContext(ctx, `
		UPDATE invitations
		SET accepted_at = now()
		WHERE id = $1 AND accepted_at IS NULL AND expires_at > now()
	`, inviteID)
	if err != nil {
		return account{}, err
	}
	rows, err := result.RowsAffected()
	if err != nil {
		return account{}, err
	}
	if rows != 1 {
		return account{}, sql.ErrNoRows
	}
	if err := tx.Commit(); err != nil {
		return account{}, err
	}
	return user, nil
}

type deviceStart struct {
	DeviceCode      string `json:"device_code"`
	UserCode        string `json:"user_code"`
	VerificationURI string `json:"verification_uri"`
	Interval        int    `json:"interval"`
	ExpiresIn       int    `json:"expires_in"`
}

func startDevice(ctx context.Context, db *sql.DB, hostname, osName string) (deviceStart, error) {
	var orgID int64
	if err := db.QueryRowContext(ctx, `SELECT id FROM organizations ORDER BY id LIMIT 1`).Scan(&orgID); err != nil {
		return deviceStart{}, err
	}
	userCode, err := newUserCode()
	if err != nil {
		return deviceStart{}, err
	}
	deviceCode, err := randomToken()
	if err != nil {
		return deviceStart{}, err
	}
	_, err = db.ExecContext(ctx, `
		INSERT INTO device_codes (org_id, user_code, device_code_hash, hostname, os, expires_at)
		VALUES ($1, $2, $3, $4, $5, now() + interval '15 minutes')
	`, orgID, userCode, hashToken(deviceCode), hostname, osName)
	if err != nil {
		return deviceStart{}, err
	}
	return deviceStart{
		DeviceCode:      deviceCode,
		UserCode:        userCode[:4] + "-" + userCode[4:],
		VerificationURI: publicURL() + "/enroll",
		Interval:        5,
		ExpiresIn:       15 * 60,
	}, nil
}

func approveDevice(ctx context.Context, db *sql.DB, user account, code string) error {
	normalized := normalizeCode(code)
	if len(normalized) != 8 {
		return sql.ErrNoRows
	}
	tx, err := db.BeginTx(ctx, nil)
	if err != nil {
		return err
	}
	defer tx.Rollback()

	var codeID int64
	var hostname, osName string
	err = tx.QueryRowContext(ctx, `
		SELECT id, hostname, os
		FROM device_codes
		WHERE org_id = $1 AND user_code = $2 AND user_id IS NULL AND expires_at > now()
	`, user.OrgID, normalized).Scan(&codeID, &hostname, &osName)
	if err != nil {
		return err
	}
	raw, err := randomToken()
	if err != nil {
		return err
	}
	if hostname == "" {
		hostname = "pending"
	}
	var hostID int64
	err = tx.QueryRowContext(ctx, `
		INSERT INTO hosts (hostname, os, org_id, user_id, token_hash, reported_at)
		VALUES ($1, $2, $3, $4, $5, now())
		RETURNING id
	`, hostname, osName, user.OrgID, user.ID, hashToken(raw)).Scan(&hostID)
	if err != nil {
		return err
	}
	_, err = tx.ExecContext(ctx, `
		UPDATE device_codes
		SET user_id = $1, host_id = $2, token = $3
		WHERE id = $4 AND user_id IS NULL
	`, user.ID, hostID, raw, codeID)
	if err != nil {
		return err
	}
	return tx.Commit()
}

func pollDevice(ctx context.Context, db *sql.DB, deviceCode string) (string, string, error) {
	var userID sql.NullInt64
	var token sql.NullString
	var expires time.Time
	err := db.QueryRowContext(ctx, `
		SELECT user_id, token, expires_at
		FROM device_codes
		WHERE device_code_hash = $1
	`, hashToken(deviceCode)).Scan(&userID, &token, &expires)
	if err != nil {
		return "", "", err
	}
	if time.Now().After(expires) {
		return "", "expired", nil
	}
	if !userID.Valid || !token.Valid || token.String == "" {
		return "", "authorization_pending", nil
	}
	raw := token.String
	_, err = db.ExecContext(ctx, `UPDATE device_codes SET token = NULL WHERE device_code_hash = $1`, hashToken(deviceCode))
	if err != nil {
		return "", "", err
	}
	return raw, "", nil
}

func saveInventoryForToken(ctx context.Context, db *sql.DB, rawToken string, payload inventoryRequest) error {
	result, err := db.ExecContext(ctx, `
		UPDATE hosts
		SET hostname = $1, os = $2, inventory = $3::jsonb, reported_at = now()
		WHERE token_hash = $4 AND NOT revoked
	`, strings.TrimSpace(payload.Hostname), payload.OS, string(payload.Tools), hashToken(rawToken))
	if err != nil {
		return err
	}
	rows, err := result.RowsAffected()
	if err != nil {
		return err
	}
	if rows != 1 {
		return sql.ErrNoRows
	}
	return nil
}

func bearerToken(r *http.Request) string {
	header := r.Header.Get("Authorization")
	const prefix = "Bearer "
	if len(header) <= len(prefix) || !strings.EqualFold(header[:len(prefix)], prefix) {
		return ""
	}
	return strings.TrimSpace(header[len(prefix):])
}

func readJSON(r *http.Request, dest any) error {
	body, err := io.ReadAll(io.LimitReader(r.Body, 1<<20))
	if err != nil {
		return err
	}
	return json.Unmarshal(body, dest)
}

func revokeHost(ctx context.Context, db *sql.DB, orgID, hostID int64) error {
	_, err := db.ExecContext(ctx, `
		UPDATE hosts
		SET revoked = true, token_hash = NULL
		WHERE id = $1 AND org_id = $2 AND user_id IS NOT NULL
	`, hostID, orgID)
	return err
}

func removeMember(ctx context.Context, db *sql.DB, orgID, userID int64) error {
	tx, err := db.BeginTx(ctx, nil)
	if err != nil {
		return err
	}
	defer tx.Rollback()
	var role string
	err = tx.QueryRowContext(ctx, `
		SELECT role FROM users WHERE id = $1 AND org_id = $2
	`, userID, orgID).Scan(&role)
	if err != nil {
		return err
	}
	if role != "member" {
		return sql.ErrNoRows
	}
	if _, err := tx.ExecContext(ctx, `
		UPDATE hosts SET revoked = true, token_hash = NULL WHERE user_id = $1
	`, userID); err != nil {
		return err
	}
	if _, err := tx.ExecContext(ctx, `DELETE FROM users WHERE id = $1`, userID); err != nil {
		return err
	}
	return tx.Commit()
}
