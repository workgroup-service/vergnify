// @author Cursor

package main

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"html/template"
	"io"
	"log"
	"net/http"
	"os"
	"regexp"
	"strconv"
	"strings"
	"time"

	_ "github.com/jackc/pgx/v5/stdlib"
)

func main() {
	if len(os.Args) > 1 && os.Args[1] == "create" {
		if err := createOwner(os.Args[2:]); err != nil {
			log.Fatal(err)
		}
		return
	}

	databaseURL := os.Getenv("DATABASE_URL")
	if databaseURL == "" {
		log.Fatal("DATABASE_URL が必要です")
	}

	db, err := sql.Open("pgx", databaseURL)
	if err != nil {
		log.Fatal(err)
	}
	defer db.Close()

	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	if err := waitForDB(ctx, db); err != nil {
		log.Fatal(err)
	}
	if err := migrate(ctx, db); err != nil {
		log.Fatal(err)
	}

	pages := template.Must(template.ParseFiles(
		"templates/dashboard.html",
		"templates/host.html",
		"templates/login.html",
		"templates/enroll.html",
		"templates/invitations.html",
	))
	mux := http.NewServeMux()
	mux.HandleFunc("GET /healthz", func(w http.ResponseWriter, r *http.Request) {
		pingCtx, pingCancel := context.WithTimeout(r.Context(), 2*time.Second)
		defer pingCancel()
		if err := db.PingContext(pingCtx); err != nil {
			http.Error(w, texts(requestLang(r)).DatabaseDown, http.StatusServiceUnavailable)
			return
		}
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte("ok"))
	})
	mux.HandleFunc("GET /login", func(w http.ResponseWriter, r *http.Request) {
		render(w, r, pages, "login.html", http.StatusOK, loginPage{})
	})
	mux.HandleFunc("POST /login", func(w http.ResponseWriter, r *http.Request) {
		email := r.FormValue("email")
		user, err := login(r.Context(), db, email, r.FormValue("password"))
		if err != nil {
			render(w, r, pages, "login.html", http.StatusUnauthorized, loginPage{
				Email: email,
				Error: true,
			})
			return
		}
		raw, err := startSession(r.Context(), db, user.ID)
		if err != nil {
			http.Error(w, texts(requestLang(r)).DatabaseDown, http.StatusServiceUnavailable)
			return
		}
		setSessionCookie(w, raw)
		http.Redirect(w, r, "/list", http.StatusSeeOther)
	})
	mux.HandleFunc("POST /invitations", func(w http.ResponseWriter, r *http.Request) {
		user, ok := requireOwner(w, r, db)
		if !ok {
			return
		}
		email := normalizeEmail(r.FormValue("email"))
		if !validEmail(email) {
			http.Error(w, "bad request", http.StatusBadRequest)
			return
		}
		if _, err := createInvite(r.Context(), db, user.OrgID, email); err != nil {
			http.Error(w, texts(requestLang(r)).DatabaseDown, http.StatusServiceUnavailable)
			return
		}
		http.Redirect(w, r, "/invitations", http.StatusSeeOther)
	})
	mux.HandleFunc("GET /invitations", func(w http.ResponseWriter, r *http.Request) {
		user, ok := requireOwner(w, r, db)
		if !ok {
			return
		}
		page, err := loadInvitePage(r.Context(), db, user.OrgID)
		if err != nil {
			http.Error(w, texts(requestLang(r)).DatabaseDown, http.StatusServiceUnavailable)
			return
		}
		render(w, r, pages, "invitations.html", http.StatusOK, page)
	})
	mux.HandleFunc("GET /invitations/accept", func(w http.ResponseWriter, r *http.Request) {
		user, err := acceptInvite(r.Context(), db, r.URL.Query().Get("token"))
		if err != nil {
			http.Error(w, texts(requestLang(r)).BadInvite, http.StatusBadRequest)
			return
		}
		raw, err := startSession(r.Context(), db, user.ID)
		if err != nil {
			http.Error(w, texts(requestLang(r)).DatabaseDown, http.StatusServiceUnavailable)
			return
		}
		setSessionCookie(w, raw)
		http.Redirect(w, r, "/enroll", http.StatusSeeOther)
	})
	mux.HandleFunc("GET /enroll", func(w http.ResponseWriter, r *http.Request) {
		if _, ok := currentUser(r.Context(), db, r); !ok {
			http.Redirect(w, r, "/login", http.StatusSeeOther)
			return
		}
		render(w, r, pages, "enroll.html", http.StatusOK, enrollPage{})
	})
	mux.HandleFunc("POST /enroll", func(w http.ResponseWriter, r *http.Request) {
		user, ok := currentUser(r.Context(), db, r)
		if !ok {
			http.Redirect(w, r, "/login", http.StatusSeeOther)
			return
		}
		if err := approveDevice(r.Context(), db, user, r.FormValue("code")); err != nil {
			render(w, r, pages, "enroll.html", http.StatusOK, enrollPage{Failed: true})
			return
		}
		if user.Role == "owner" {
			http.Redirect(w, r, "/list", http.StatusSeeOther)
			return
		}
		render(w, r, pages, "enroll.html", http.StatusOK, enrollPage{OK: true})
	})
	mux.HandleFunc("POST /api/device/code", func(w http.ResponseWriter, r *http.Request) {
		var body struct {
			Hostname string `json:"hostname"`
			OS       string `json:"os"`
		}
		if err := readJSON(r, &body); err != nil {
			http.Error(w, "bad request", http.StatusBadRequest)
			return
		}
		started, err := startDevice(r.Context(), db, strings.TrimSpace(body.Hostname), body.OS)
		if err != nil {
			http.Error(w, texts(requestLang(r)).DatabaseDown, http.StatusServiceUnavailable)
			return
		}
		writeJSON(w, http.StatusOK, started)
	})
	mux.HandleFunc("POST /api/device/token", func(w http.ResponseWriter, r *http.Request) {
		var body struct {
			DeviceCode string `json:"device_code"`
		}
		if err := readJSON(r, &body); err != nil || body.DeviceCode == "" {
			http.Error(w, "bad request", http.StatusBadRequest)
			return
		}
		token, state, err := pollDevice(r.Context(), db, body.DeviceCode)
		if err != nil {
			http.Error(w, "bad request", http.StatusBadRequest)
			return
		}
		if state != "" {
			writeJSON(w, http.StatusBadRequest, map[string]string{"error": state})
			return
		}
		writeJSON(w, http.StatusOK, map[string]string{"token": token})
	})
	mux.HandleFunc("POST /api/inventory", func(w http.ResponseWriter, r *http.Request) {
		token := bearerToken(r)
		if token == "" {
			http.Error(w, "unauthorized", http.StatusUnauthorized)
			return
		}
		body, err := io.ReadAll(io.LimitReader(r.Body, 1<<20))
		if err != nil {
			http.Error(w, "bad request", http.StatusBadRequest)
			return
		}
		var payload inventoryRequest
		if err := json.Unmarshal(body, &payload); err != nil || !validInventory(payload) {
			http.Error(w, "bad request", http.StatusBadRequest)
			return
		}
		saveCtx, saveCancel := context.WithTimeout(r.Context(), 5*time.Second)
		defer saveCancel()
		if err := saveInventoryForToken(saveCtx, db, token, payload); err != nil {
			http.Error(w, "unauthorized", http.StatusUnauthorized)
			return
		}
		w.WriteHeader(http.StatusNoContent)
	})
	mux.HandleFunc("GET /", func(w http.ResponseWriter, r *http.Request) {
		http.Redirect(w, r, "/list", http.StatusSeeOther)
	})
	mux.HandleFunc("GET /list", func(w http.ResponseWriter, r *http.Request) {
		user, ok := requireOwner(w, r, db)
		if !ok {
			return
		}
		hosts, err := loadHosts(r.Context(), db, user.OrgID)
		if err != nil {
			http.Error(w, texts(requestLang(r)).DatabaseDown, http.StatusServiceUnavailable)
			return
		}
		render(w, r, pages, "dashboard.html", http.StatusOK, pageData{Hosts: hosts})
	})
	mux.HandleFunc("POST /hosts/{id}/revoke", func(w http.ResponseWriter, r *http.Request) {
		user, ok := requireOwner(w, r, db)
		if !ok {
			return
		}
		id, err := strconv.ParseInt(r.PathValue("id"), 10, 64)
		if err != nil || id < 1 {
			http.NotFound(w, r)
			return
		}
		if err := revokeHost(r.Context(), db, user.OrgID, id); err != nil {
			http.Error(w, texts(requestLang(r)).DatabaseDown, http.StatusServiceUnavailable)
			return
		}
		http.Redirect(w, r, "/list", http.StatusSeeOther)
	})
	mux.HandleFunc("POST /members/{id}/remove", func(w http.ResponseWriter, r *http.Request) {
		user, ok := requireOwner(w, r, db)
		if !ok {
			return
		}
		id, err := strconv.ParseInt(r.PathValue("id"), 10, 64)
		if err != nil || id < 1 {
			http.NotFound(w, r)
			return
		}
		if err := removeMember(r.Context(), db, user.OrgID, id); err != nil {
			http.Error(w, "bad request", http.StatusBadRequest)
			return
		}
		http.Redirect(w, r, "/invitations", http.StatusSeeOther)
	})
	mux.HandleFunc("GET /hosts/{id}", func(w http.ResponseWriter, r *http.Request) {
		user, ok := requireOwner(w, r, db)
		if !ok {
			return
		}
		id, err := strconv.ParseInt(r.PathValue("id"), 10, 64)
		if err != nil || id < 1 {
			http.NotFound(w, r)
			return
		}
		detail, err := loadHost(r.Context(), db, user.OrgID, id, requestLang(r))
		if errors.Is(err, sql.ErrNoRows) {
			http.NotFound(w, r)
			return
		}
		if err != nil {
			http.Error(w, texts(requestLang(r)).DatabaseDown, http.StatusServiceUnavailable)
			return
		}
		render(w, r, pages, "host.html", http.StatusOK, detail)
	})

	addr := ":8080"
	log.Printf("admin listening on %s", addr)
	log.Fatal(http.ListenAndServe(addr, mux))
}

type inventoryRequest struct {
	Hostname string          `json:"hostname"`
	OS       string          `json:"os"`
	Tools    json.RawMessage `json:"tools"`
}

type toolJSON struct {
	Name           string `json:"name"`
	CurrentVersion string `json:"current_version"`
	Line           string `json:"line"`
	LatestVersion  string `json:"latest_version"`
	Newer          []struct {
		Label  string `json:"label"`
		Latest string `json:"latest"`
	} `json:"newer"`
	Status          string `json:"status"`
	Vulnerabilities []struct {
		CveID    string `json:"cve_id"`
		Severity string `json:"severity"`
		Title    string `json:"title"`
	} `json:"vulnerabilities"`
}

type hostView struct {
	ID         int64
	Email      string
	Hostname   string
	OS         string
	ToolCount  int
	AlertCount int
	ReportedAt string
	Stale      bool
}

type inviteView struct {
	Email     string
	ExpiresAt string
	AcceptURL string
}

type memberView struct {
	ID    int64
	Email string
	Owner bool
}

type enrollPage struct {
	OK     bool
	Failed bool
}

type loginPage struct {
	Email string
	Error bool
}

type vulnView struct {
	ID       string
	Severity string
	Title    string
	URL      template.URL
}

type toolView struct {
	Name            string
	CurrentVersion  string
	Line            string
	LatestVersion   string
	Status          string
	Newer           string
	Vulnerabilities []vulnView
}

type hostDetail struct {
	Email      string
	Hostname   string
	OS         string
	ReportedAt string
	Stale      bool
	Tools      []toolView
}

type pageData struct {
	Hosts []hostView
}

type invitePage struct {
	Invites []inviteView
	Members []memberView
}

func validInventory(payload inventoryRequest) bool {
	name := strings.TrimSpace(payload.Hostname)
	if name == "" || len(name) > 255 || len(payload.OS) > 64 {
		return false
	}
	var tools []json.RawMessage
	return json.Unmarshal(payload.Tools, &tools) == nil
}

func requireOwner(w http.ResponseWriter, r *http.Request, db *sql.DB) (account, bool) {
	user, ok := currentUser(r.Context(), db, r)
	if !ok || user.Role != "owner" {
		http.Redirect(w, r, "/login", http.StatusSeeOther)
		return account{}, false
	}
	return user, true
}

func render(w http.ResponseWriter, r *http.Request, pages *template.Template, name string, status int, data any) {
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	w.WriteHeader(status)
	if err := pages.ExecuteTemplate(w, name, struct {
		T    copy
		Data any
	}{texts(requestLang(r)), data}); err != nil {
		http.Error(w, "template failed", http.StatusInternalServerError)
	}
}

func writeJSON(w http.ResponseWriter, status int, body any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(body)
}

func loadInvitePage(ctx context.Context, db *sql.DB, orgID int64) (invitePage, error) {
	invites, err := loadInvites(ctx, db, orgID)
	if err != nil {
		return invitePage{}, err
	}
	members, err := loadMembers(ctx, db, orgID)
	if err != nil {
		return invitePage{}, err
	}
	return invitePage{Invites: invites, Members: members}, nil
}

func loadHosts(ctx context.Context, db *sql.DB, orgID int64) ([]hostView, error) {
	rows, err := db.QueryContext(ctx, `
		SELECT hosts.id, users.email, hosts.hostname, hosts.os, hosts.inventory, hosts.reported_at
		FROM hosts
		JOIN users ON users.id = hosts.user_id
		WHERE hosts.org_id = $1 AND hosts.user_id IS NOT NULL AND NOT hosts.revoked
		ORDER BY hosts.reported_at DESC
	`, orgID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var hosts []hostView
	for rows.Next() {
		var view hostView
		var inventory []byte
		var reported time.Time
		if err := rows.Scan(&view.ID, &view.Email, &view.Hostname, &view.OS, &inventory, &reported); err != nil {
			return nil, err
		}
		view.ReportedAt = reported.UTC().Format(time.RFC3339)
		view.Stale = stale(reported)
		tools, err := decodeTools(inventory)
		if err != nil {
			return nil, err
		}
		view.ToolCount = len(tools)
		view.AlertCount = alertCount(tools)
		hosts = append(hosts, view)
	}
	return hosts, rows.Err()
}

func loadInvites(ctx context.Context, db *sql.DB, orgID int64) ([]inviteView, error) {
	rows, err := db.QueryContext(ctx, `
		SELECT email, expires_at
		FROM invitations
		WHERE org_id = $1 AND accepted_at IS NULL AND expires_at > now()
		ORDER BY expires_at
	`, orgID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var invites []inviteView
	for rows.Next() {
		var view inviteView
		var expires time.Time
		if err := rows.Scan(&view.Email, &expires); err != nil {
			return nil, err
		}
		view.ExpiresAt = expires.UTC().Format(time.RFC3339)
		invites = append(invites, view)
	}
	return invites, rows.Err()
}

func loadMembers(ctx context.Context, db *sql.DB, orgID int64) ([]memberView, error) {
	rows, err := db.QueryContext(ctx, `
		SELECT id, email, role FROM users WHERE org_id = $1 ORDER BY email
	`, orgID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var members []memberView
	for rows.Next() {
		var view memberView
		var role string
		if err := rows.Scan(&view.ID, &view.Email, &role); err != nil {
			return nil, err
		}
		view.Owner = role == "owner"
		members = append(members, view)
	}
	return members, rows.Err()
}

func loadHost(ctx context.Context, db *sql.DB, orgID, id int64, lang uiLang) (hostDetail, error) {
	var detail hostDetail
	var inventory []byte
	var reported time.Time
	err := db.QueryRowContext(ctx, `
		SELECT users.email, hosts.hostname, hosts.os, hosts.inventory, hosts.reported_at
		FROM hosts
		JOIN users ON users.id = hosts.user_id
		WHERE hosts.id = $1 AND hosts.org_id = $2 AND hosts.user_id IS NOT NULL AND NOT hosts.revoked
	`, id, orgID).Scan(&detail.Email, &detail.Hostname, &detail.OS, &inventory, &reported)
	if err != nil {
		return hostDetail{}, err
	}
	detail.ReportedAt = reported.UTC().Format(time.RFC3339)
	detail.Stale = stale(reported)
	tools, err := decodeTools(inventory)
	if err != nil {
		return hostDetail{}, err
	}
	detail.Tools = make([]toolView, 0, len(tools))
	for _, tool := range tools {
		view := toolView{
			Name:           tool.Name,
			CurrentVersion: tool.CurrentVersion,
			Line:           tool.Line,
			LatestVersion:  tool.LatestVersion,
			Status:         statusLabel(tool.Status, lang),
			Newer:          newerLabel(tool),
		}
		for _, vuln := range tool.Vulnerabilities {
			view.Vulnerabilities = append(view.Vulnerabilities, vulnView{
				ID:       vuln.CveID,
				Severity: severityLabel(vuln.Severity, lang),
				Title:    vuln.Title,
				URL:      vulnURL(vuln.CveID),
			})
		}
		detail.Tools = append(detail.Tools, view)
	}
	return detail, nil
}

func decodeTools(inventory []byte) ([]toolJSON, error) {
	var tools []toolJSON
	if err := json.Unmarshal(inventory, &tools); err != nil {
		return nil, err
	}
	return tools, nil
}

func alertCount(tools []toolJSON) int {
	count := 0
	for _, tool := range tools {
		for _, vuln := range tool.Vulnerabilities {
			if vuln.Severity == "high" || vuln.Severity == "critical" {
				count++
			}
		}
	}
	return count
}

func newerLabel(tool toolJSON) string {
	parts := make([]string, 0, len(tool.Newer))
	for _, line := range tool.Newer {
		parts = append(parts, strings.TrimSpace(line.Label+" "+line.Latest))
	}
	return strings.Join(parts, ", ")
}

var vulnIDPattern = regexp.MustCompile(`^[A-Za-z0-9._-]+$`)

func vulnURL(id string) template.URL {
	if !vulnIDPattern.MatchString(id) {
		return ""
	}
	return template.URL("https://osv.dev/vulnerability/" + id)
}

func waitForDB(ctx context.Context, db *sql.DB) error {
	var err error
	for {
		err = db.PingContext(ctx)
		if err == nil {
			return nil
		}
		select {
		case <-ctx.Done():
			return err
		case <-time.After(time.Second):
		}
	}
}
