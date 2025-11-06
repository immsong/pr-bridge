use serde::{Deserialize, Serialize};
use serde_json::Value;

// =============================================================================
// 기본 메시지 구조
// =============================================================================

/// 클라이언트 → 서버 메시지
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientMessage {
    /// 요청 ID (응답 매칭용, 선택사항)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// 메시지 타입 및 데이터
    #[serde(flatten)]
    pub payload: ClientMessageType,
}

/// 서버 → 클라이언트 메시지
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerMessage {
    /// 원래 요청 ID (응답인 경우)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// 메시지 타입 및 데이터
    #[serde(flatten)]
    pub payload: ServerMessageType,
}

// =============================================================================
// 클라이언트 메시지 타입
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum ClientMessageType {
    // -------------------------------------------------------------------------
    // 저장소 관리
    // -------------------------------------------------------------------------
    /// 저장소 등록
    AddRepository {
        owner: String,
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        poll_interval_seconds: Option<i32>,
    },

    /// 저장소 수정
    UpdateRepository {
        repo_id: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_active: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        poll_interval_seconds: Option<i32>,
    },

    /// 저장소 삭제
    DeleteRepository { repo_id: i32 },

    // -------------------------------------------------------------------------
    // Jenkins 연동 설정
    // -------------------------------------------------------------------------
    /// Jenkins 매핑 추가/수정
    SetJenkinsMapping {
        repo_id: i32,
        jenkins_job_name: String,
        jenkins_url: String,
        auto_trigger: bool,
    },

    /// Jenkins 매핑 삭제
    DeleteJenkinsMapping { repo_id: i32 },

    // -------------------------------------------------------------------------
    // 빌드 제어
    // -------------------------------------------------------------------------
    /// 수동 빌드 트리거
    TriggerBuild { pr_id: i32 },

    // -------------------------------------------------------------------------
    // 시스템 설정
    // -------------------------------------------------------------------------
    /// 시스템 설정 변경
    UpdateSystemSetting { key: String, value: String },

    // -------------------------------------------------------------------------
    // 조회 (Query)
    // -------------------------------------------------------------------------
    /// 저장소 목록 조회
    GetRepositories,

    /// 특정 저장소 조회
    GetRepository { repo_id: i32 },

    /// PR 목록 조회
    GetPullRequests {
        repo_id: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<String>, // "open", "closed", "merged"
    },

    /// 빌드 이력 조회
    GetBuildHistory { pr_id: i32 },

    /// 시스템 설정 조회
    GetSystemSettings,

    /// 폴링 이력 조회
    GetPollingHistory {
        repo_id: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        limit: Option<i32>,
    },
}

// =============================================================================
// 서버 메시지 타입
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum ServerMessageType {
    // -------------------------------------------------------------------------
    // 응답 (Response)
    // -------------------------------------------------------------------------
    /// 저장소 목록
    Repositories { repositories: Vec<Value> },

    /// 저장소 상세
    Repository { repository: Value },

    /// PR 목록
    PullRequests { pull_requests: Vec<Value> },

    /// 빌드 이력
    BuildHistory { builds: Vec<Value> },

    /// 시스템 설정
    SystemSettings { settings: Value },

    /// 폴링 이력
    PollingHistory { history: Vec<Value> },

    /// 성공 응답
    Success {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<Value>,
    },

    // -------------------------------------------------------------------------
    // 실시간 이벤트 (Event)
    // -------------------------------------------------------------------------
    /// PR 업데이트 알림
    PrUpdated {
        repo_id: i32,
        pr_number: i32,
        pr_id: i32,
        status: String,
        head_sha: String,
    },

    /// 새 PR 감지
    PrOpened {
        repo_id: i32,
        pr_number: i32,
        pr_id: i32,
        title: String,
        author: Option<String>,
    },

    /// PR 닫힘
    PrClosed {
        repo_id: i32,
        pr_number: i32,
        pr_id: i32,
        merged: bool,
    },

    /// 빌드 트리거 성공
    BuildTriggered {
        pr_id: i32,
        commit_sha: String,
        jenkins_build_number: Option<i32>,
        jenkins_build_url: Option<String>,
    },

    /// 빌드 트리거 실패
    BuildTriggerFailed {
        pr_id: i32,
        commit_sha: String,
        error: String,
    },

    /// 브랜치 동기화 완료
    BranchesSynced {
        repo_id: i32,
        added: i32,
        updated: i32,
        deleted: i32,
    },

    /// 태그 동기화 완료
    TagsSynced { repo_id: i32, added: i32 },

    /// 폴링 완료
    PollingCompleted {
        repo_id: i32,
        polled_at: String, // ISO 8601 timestamp
    },

    /// 설정 변경 알림
    SettingsChanged {
        changed_by: String,
        keys: Vec<String>,
    },

    // -------------------------------------------------------------------------
    // 에러
    // -------------------------------------------------------------------------
    /// 에러 응답
    Error {
        code: ErrorCode,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        details: Option<Value>,
    },

    // -------------------------------------------------------------------------
    // 기타
    // -------------------------------------------------------------------------
    /// Pong (Ping 응답)
    Pong,
}

// =============================================================================
// 에러 코드
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    /// 잘못된 요청
    InvalidRequest,

    /// 리소스를 찾을 수 없음
    NotFound,

    /// 권한 없음
    Unauthorized,

    /// DB 에러
    DatabaseError,

    /// GitHub API 에러
    GithubApiError,

    /// Jenkins API 에러
    JenkinsApiError,

    /// 내부 서버 에러
    InternalError,

    /// 중복된 리소스
    AlreadyExists,

    /// 유효성 검증 실패
    ValidationError,
}
