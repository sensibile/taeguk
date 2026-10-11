defmodule TaegukWeb.StatusController do
  use Phoenix.Controller, formats: [:html, :json]
  def health(conn, _), do: json(conn, %{status: "ok", scope: "process_response_only"})

  def capabilities(conn, _) do
    json(conn, %{
      phase: "local_workflow_state",
      persistence: Application.get_env(:taeguk, :state_enabled, false),
      scheduling: false,
      deployment: false,
      mcp: false,
      cli: ["measure", "verify", "audit", "state"]
    })
  end

  def index(conn, _) do
    html(conn, """
    <!doctype html><html lang="ko"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Taeguk</title></head>
    <body><main><h1>Taeguk</h1><p>목적의 가설에서 작은 변경, 검증, 실제 효과와 회고까지.</p>
    <h2>초기 구성</h2><p>에이전트·스킬과 측정·증거 CLI를 실제 제품 개발에서 점검합니다.</p>
    <p>현재 작업 저장 API는 SQLite 설정 후 사용할 수 있습니다. 예약 실행과 자동 배포는 제공하지 않습니다.</p>
    <nav aria-label="API"><a href="/api/health">응답 확인</a> · <a href="/api/capabilities">기능 상태</a></nav></main></body></html>
    """)
  end
end
