-- =============================================================================
-- PR Bridge 초기 스키마
-- =============================================================================

-- -----------------------------------------------------------------------------
-- 저장소 테이블
-- 추적할 GitHub 저장소 목록
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS repositories (
    id SERIAL PRIMARY KEY,
    owner VARCHAR(255) NOT NULL,
    name VARCHAR(255) NOT NULL,
    poll_interval_seconds INTEGER DEFAULT 60,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE(owner, name)
);

-- -----------------------------------------------------------------------------
-- 저장소 폴링 이력
-- GitHub API 폴링 실행 기록 (모니터링 및 디버깅용)
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS repositories_polling_history (
    id SERIAL PRIMARY KEY,
    repository_id INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    polled_at TIMESTAMP NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE(repository_id, polled_at)
);

-- -----------------------------------------------------------------------------
-- 브랜치 테이블
-- 저장소의 브랜치 정보 (origin 기준)
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS branches (
    id SERIAL PRIMARY KEY,
    repository_id INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    head_sha VARCHAR(64) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE(repository_id, name)
);

-- -----------------------------------------------------------------------------
-- 태그 테이블
-- 저장소의 태그/릴리즈 정보
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS tags (
    id SERIAL PRIMARY KEY,
    repository_id INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    commit_sha VARCHAR(64) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE(repository_id, name)
);

-- -----------------------------------------------------------------------------
-- Pull Request 테이블
-- GitHub PR 정보 및 상태 추적
-- source_branch_id, target_branch_id는 branches 테이블 참조
-- head_sha는 중복 빌드 트리거 방지용
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS pull_requests (
    id SERIAL PRIMARY KEY,
    repository_id INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    pr_number INTEGER NOT NULL,
    title TEXT NOT NULL,
    author VARCHAR(255),
    source_branch_id INTEGER NOT NULL REFERENCES branches(id) ON DELETE RESTRICT,
    target_branch_id INTEGER NOT NULL REFERENCES branches(id) ON DELETE RESTRICT,
    head_sha VARCHAR(64) NOT NULL,
    status VARCHAR(50) NOT NULL,
    last_polled_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE(repository_id, pr_number)
);

-- -----------------------------------------------------------------------------
-- Jenkins 매핑 테이블
-- 저장소별 Jenkins Job 설정
-- 하나의 저장소는 하나의 Jenkins Job과 매핑됨
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS jenkins_mappings (
    id SERIAL PRIMARY KEY,
    repository_id INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    jenkins_job_name VARCHAR(255) NOT NULL,
    jenkins_url VARCHAR(512),
    auto_trigger BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE(repository_id)
);

-- -----------------------------------------------------------------------------
-- 빌드 트리거 이력
-- Jenkins 빌드 트리거 실행 기록
-- 같은 PR의 같은 커밋은 한 번만 트리거되도록 UNIQUE 제약 조건 설정
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS build_triggers (
    id SERIAL PRIMARY KEY,
    pull_request_id INTEGER NOT NULL REFERENCES pull_requests(id) ON DELETE CASCADE,
    commit_sha VARCHAR(64) NOT NULL,
    jenkins_build_number INTEGER,
    jenkins_build_url VARCHAR(512),
    trigger_status VARCHAR(50) NOT NULL,
    trigger_message TEXT,
    triggered_at TIMESTAMP NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMP,
    UNIQUE(pull_request_id, commit_sha)
);

-- -----------------------------------------------------------------------------
-- 시스템 설정 테이블
-- 전역 설정 Key-Value 저장소
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS system_settings (
    key VARCHAR(255) PRIMARY KEY,
    value TEXT NOT NULL,
    description TEXT,
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

-- 기본 설정 삽입
INSERT INTO system_settings (key, value, description) VALUES
    ('github_api_poll_interval', '300', 'GitHub API 폴링 주기 (초)'),
    ('sync_refs_interval', '180', '브랜치/태그 동기화 주기 (초)')
ON CONFLICT (key) DO NOTHING;