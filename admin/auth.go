// @author Cursor

package main

import (
	"context"
	"crypto/rand"
	"crypto/sha256"
	"database/sql"
	"encoding/base64"
	"encoding/hex"
	"errors"
	"flag"
	"fmt"
	"net/http"
	"os"
	"strings"
	"time"

	"golang.org/x/crypto/bcrypt"
)

const sessionCookie = "vergnify_session"

type account struct {
	ID    int64
	OrgID int64
	Email string
	Role  string
}

func createOwner(args []string) error {
	flags := flag.NewFlagSet("create", flag.ContinueOnError)
	email := flags.String("email", "", "owner email")
	password := flags.String("password", "", "owner password")
	if err := flags.Parse(args); err != nil {
		return err
	}
	address := normalizeEmail(*email)
	if !validEmail(address) || *password == "" {
		return errors.New("admin create --email EMAIL --password PASSWORD")
	}

	databaseURL := os.Getenv("DATABASE_URL")
	if databaseURL == "" {
		return errors.New("DATABASE_URL が必要です")
	}
	db, err := sql.Open("pgx", databaseURL)
	if err != nil {
		return err
	}
	defer db.Close()

	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	if err := waitForDB(ctx, db); err != nil {
		return err
	}
	if err := migrate(ctx, db); err != nil {
		return err
	}

	var owners int
	if err := db.QueryRowContext(ctx, `SELECT COUNT(*) FROM users WHERE role = 'owner'`).Scan(&owners); err != nil {
		return err
	}
	if owners > 0 {
		return errors.New("Owner は既にいます")
	}

	hash, err := bcrypt.GenerateFromPassword([]byte(*password), bcrypt.DefaultCost)
	if err != nil {
		return err
	}
	tx, err := db.BeginTx(ctx, nil)
	if err != nil {
		return err
	}
	defer tx.Rollback()

	var orgID int64
	err = tx.QueryRowContext(ctx, `SELECT id FROM organizations ORDER BY id LIMIT 1`).Scan(&orgID)
	if errors.Is(err, sql.ErrNoRows) {
		err = tx.QueryRowContext(ctx, `INSERT INTO organizations (name) VALUES ('local') RETURNING id`).Scan(&orgID)
	}
	if err != nil {
		return err
	}
	_, err = tx.ExecContext(ctx, `
		INSERT INTO users (org_id, email, password_hash, role)
		VALUES ($1, $2, $3, 'owner')
	`, orgID, address, string(hash))
	if err != nil {
		return err
	}
	if err := tx.Commit(); err != nil {
		return err
	}
	fmt.Printf("Owner を作りました (%s)\n", address)
	return nil
}

func normalizeEmail(email string) string {
	return strings.ToLower(strings.TrimSpace(email))
}

func validEmail(email string) bool {
	if len(email) < 3 || len(email) > 254 || strings.ContainsAny(email, " \t") {
		return false
	}
	at := strings.IndexByte(email, '@')
	return at > 0 && at < len(email)-1 && !strings.Contains(email[at+1:], "@")
}

func hashToken(raw string) string {
	sum := sha256.Sum256([]byte(raw))
	return hex.EncodeToString(sum[:])
}

func randomToken() (string, error) {
	buf := make([]byte, 32)
	if _, err := rand.Read(buf); err != nil {
		return "", err
	}
	return base64.RawURLEncoding.EncodeToString(buf), nil
}

func login(ctx context.Context, db *sql.DB, email, password string) (account, error) {
	var user account
	var hash string
	err := db.QueryRowContext(ctx, `
		SELECT id, org_id, email, role, password_hash
		FROM users
		WHERE email = $1 AND role = 'owner'
	`, normalizeEmail(email)).Scan(&user.ID, &user.OrgID, &user.Email, &user.Role, &hash)
	if err != nil {
		return account{}, err
	}
	if bcrypt.CompareHashAndPassword([]byte(hash), []byte(password)) != nil {
		return account{}, sql.ErrNoRows
	}
	return user, nil
}

func startSession(ctx context.Context, db *sql.DB, userID int64) (string, error) {
	raw, err := randomToken()
	if err != nil {
		return "", err
	}
	_, err = db.ExecContext(ctx, `
		INSERT INTO sessions (user_id, token_hash, expires_at)
		VALUES ($1, $2, now() + interval '14 days')
	`, userID, hashToken(raw))
	if err != nil {
		return "", err
	}
	return raw, nil
}

func setSessionCookie(w http.ResponseWriter, raw string) {
	http.SetCookie(w, &http.Cookie{
		Name:     sessionCookie,
		Value:    raw,
		Path:     "/",
		HttpOnly: true,
		SameSite: http.SameSiteLaxMode,
		MaxAge:   14 * 24 * 60 * 60,
	})
}

func currentUser(ctx context.Context, db *sql.DB, r *http.Request) (account, bool) {
	cookie, err := r.Cookie(sessionCookie)
	if err != nil || cookie.Value == "" {
		return account{}, false
	}
	var user account
	err = db.QueryRowContext(ctx, `
		SELECT users.id, users.org_id, users.email, users.role
		FROM sessions
		JOIN users ON users.id = sessions.user_id
		WHERE sessions.token_hash = $1 AND sessions.expires_at > now()
	`, hashToken(cookie.Value)).Scan(&user.ID, &user.OrgID, &user.Email, &user.Role)
	if err != nil {
		return account{}, false
	}
	return user, true
}

func stale(reported time.Time) bool {
	return time.Since(reported) > time.Hour
}
