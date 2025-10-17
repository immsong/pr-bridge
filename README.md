# PR Bridge

GitHub Pull Request를 감지하여 Jenkins 빌드를 자동 트리거하는 관리 서버입니다.

## 개요

PR Bridge는 GitHub PR을 주기적으로 폴링하고 Jenkins 서버로 빌드 트리거를 전송하는 서버 애플리케이션입니다. 
WebSocket 기반 실시간 통신을 통해 프론트엔드에서 설정을 관리할 수 있습니다.

## 주요 기능

### 핵심 기능
- GitHub PR 폴링 및 변경 감지
- Jenkins 빌드 자동 트리거
- 중복 트리거 방지 (커밋 SHA 추적)

### 설정 관리
- 폴링 주기 동적 조정
- 저장소별 설정 관리
- Jenkins 연동 설정

### 데이터 관리
- 설정 정보 DB 저장
- PR 처리 이력 추적
- 트리거 로그 저장