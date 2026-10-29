// @author Cursor

package main

import (
	"net/http"
	"strings"
)

type uiLang int

const (
	langEn uiLang = iota
	langJa
)

type copy struct {
	Lang            string
	LoginTitle      string
	Email           string
	Password        string
	LogIn           string
	BadLogin        string
	InviteEmail     string
	Invitations     string
	Invite          string
	PendingInvites  string
	Members         string
	OwnerLabel      string
	Remove          string
	Device          string
	Tools           string
	Alerts          string
	Received        string
	Stale           string
	NoDevices       string
	DeviceList      string
	Installed       string
	Line            string
	LatestInLine    string
	Status          string
	Vulnerabilities string
	NoVulns         string
	NewerLines      string
	NoTools         string
	EnrollTitle     string
	EnrollHint      string
	Approve         string
	Enrolled        string
	ClosePage       string
	BadCode         string
	BadInvite       string
	DatabaseDown    string
}

func requestLang(r *http.Request) uiLang {
	header := r.Header.Get("Accept-Language")
	if header == "" {
		return langEn
	}
	first := header
	if i := strings.IndexByte(first, ','); i >= 0 {
		first = first[:i]
	}
	if i := strings.IndexByte(first, ';'); i >= 0 {
		first = first[:i]
	}
	tag := strings.ToLower(strings.TrimSpace(strings.ReplaceAll(first, "_", "-")))
	if tag == "ja" || strings.HasPrefix(tag, "ja-") {
		return langJa
	}
	return langEn
}

func texts(lang uiLang) copy {
	if lang == langJa {
		return copy{
			Lang:            "ja",
			LoginTitle:      "ログイン · Vergnify Admin",
			Email:           "メール",
			Password:        "パスワード",
			LogIn:           "ログイン",
			BadLogin:        "メールアドレスまたはパスワードが違います",
			InviteEmail:     "招待するメール",
			Invitations:     "招待",
			Invite:          "招待",
			PendingInvites:  "未承諾の招待",
			Members:         "メンバー",
			OwnerLabel:      "(管理者(Owner))",
			Remove:          "外す",
			Device:          "端末",
			Tools:           "ツール",
			Alerts:          "要注意 CVE",
			Received:        "受信",
			Stale:           "古い",
			NoDevices:       "端末はまだありません。",
			DeviceList:      "端末一覧",
			Installed:       "インストール済み",
			Line:            "系列",
			LatestInLine:    "系列の最新",
			Status:          "状態",
			Vulnerabilities: "脆弱性",
			NoVulns:         "なし",
			NewerLines:      "より新しい系列:",
			NoTools:         "インストール済みのツールはまだありません。",
			EnrollTitle:     "端末の登録",
			EnrollHint:      "エージェントが表示したコードを入力します。承認すると、このアカウントの端末として在庫を受け取ります。",
			Approve:         "承認",
			Enrolled:        "端末を登録しました。エージェントが在庫を送り始めます。",
			ClosePage:       "このページを閉じてください",
			BadCode:         "コードが無効です。",
			BadInvite:       "招待は無効です",
			DatabaseDown:    "データベースに接続できません",
		}
	}
	return copy{
		Lang:            "en",
		LoginTitle:      "Log in · Vergnify Admin",
		Email:           "Email",
		Password:        "Password",
		LogIn:           "Log in",
		BadLogin:        "The email address or password is incorrect",
		InviteEmail:     "Email to invite",
		Invitations:     "Invitations",
		Invite:          "Invite",
		PendingInvites:  "Pending invitations",
		Members:         "Members",
		OwnerLabel:      "(Administrator (Owner))",
		Remove:          "Remove",
		Device:          "Device",
		Tools:           "Tools",
		Alerts:          "High CVEs",
		Received:        "Received",
		Stale:           "Stale",
		NoDevices:       "No devices yet.",
		DeviceList:      "Devices",
		Installed:       "Installed",
		Line:            "Line",
		LatestInLine:    "Latest in line",
		Status:          "Status",
		Vulnerabilities: "Vulnerabilities",
		NoVulns:         "None",
		NewerLines:      "Newer lines:",
		NoTools:         "No installed tools yet.",
		EnrollTitle:     "Register a device",
		EnrollHint:      "Enter the code shown by the agent. Approving it receives inventory for this account's device.",
		Approve:         "Approve",
		Enrolled:        "The device is registered. The agent will start sending inventory.",
		ClosePage:       "Please close this page.",
		BadCode:         "The code is invalid.",
		BadInvite:       "This invitation is invalid",
		DatabaseDown:    "The database is unavailable",
	}
}

func statusLabel(status string, lang uiLang) string {
	labels := map[uiLang]map[string]string{
		langJa: {
			"up_to_date":  "最新",
			"outdated":    "更新あり",
			"unsupported": "サポート終了",
		},
		langEn: {
			"up_to_date":  "Up to date",
			"outdated":    "Update available",
			"unsupported": "Unsupported",
		},
	}
	if label, ok := labels[lang][status]; ok {
		return label
	}
	if lang == langJa {
		return "不明"
	}
	return "Unknown"
}

func severityLabel(severity string, lang uiLang) string {
	labels := map[uiLang]map[string]string{
		langJa: {"critical": "重大", "high": "高", "medium": "中", "low": "低"},
		langEn: {"critical": "Critical", "high": "High", "medium": "Medium", "low": "Low"},
	}
	if label, ok := labels[lang][severity]; ok {
		return label
	}
	return severity
}
