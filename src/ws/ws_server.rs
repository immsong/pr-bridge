use std::{collections::HashMap, sync::Arc};
use tokio::{net::TcpStream, sync::Mutex};
// use crate::filter::{FilterAlgorithm, FilterAlgorithmType, FilterState, NoneFilter};
// use crate::handler::{UdpHandler, UdpHandlerResult, WsHandler};
// use crate::message::{
//     BasicInfo, DetectionInfo, DetectionResult, DeviceID, ErrorCode, NotifyData, ResponseData,
//     WarningArea, WsMessage,
// };
// use crate::utils;
// use crate::ws::types::{ALL_CHANNELS_SUMMARY, Client, DeviceDetectionState};
// use futures_util::{SinkExt, StreamExt};
// use once_cell::sync::Lazy;
// use regex::Regex;
// use serde_json;
// use std::collections::hash_map::Entry;
// use std::collections::{HashMap, HashSet};
// use std::net::SocketAddr;
// use std::sync::{
//     Arc,
//     mpsc::{Receiver, SyncSender},
// };
// use tokio::net::TcpListener;
// use tokio::sync::Mutex;
use tokio_tungstenite::{WebSocketStream, accept_async, tungstenite::Message};
use uuid::Uuid;
// use tracing::{error, info};
// use uuid::Uuid;

type WebSocketWriter = futures_util::stream::SplitSink<WebSocketStream<TcpStream>, Message>;

#[derive(Debug, Clone)]
pub struct Client {
    /// WebSocket 쓰기 스트림
    pub write: Arc<Mutex<WebSocketWriter>>,
}

pub struct WsServer {
    clients: Arc<Mutex<HashMap<Uuid, Client>>>,
}

impl WsServer {
    pub fn new() -> Self {
        WsServer {
            clients: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn find_available_port(start_port: u16, max_attempts: u16) -> u16 {
        let mut ret: u16 = start_port;
        for port in start_port..start_port + max_attempts {
            let addr = SocketAddr::from(([0, 0, 0, 0], port));
            match TcpListener::bind(addr).await {
                Ok(_) => {
                    ret = port;
                    break;
                }
                Err(_) => {
                    // 포트가 이미 사용 중이므로 다음 포트 시도
                    continue;
                }
            }
        }
        return ret;
    }

    /// WebSocket 서버를 실행
    ///
    /// 포트 5555부터 시작하여 사용 가능한 포트를 검색하고,
    /// 클라이언트 연결을 수락하며 메시지를 처리
    /// 각 클라이언트 연결은 별도의 비동기 태스크로 처리
    pub async fn run(&self) {
        let port = Self::find_available_port(5555, 5).await;
        let addr = format!("0.0.0.0:{}", port);
        info!("WebSocket Server Start: {}", addr);
        let listener = TcpListener::bind(addr).await.unwrap();

        while let Ok((stream, addr)) = listener.accept().await {
            // WebSocket 연결로 업그레이드
            let ws_stream = match accept_async(stream).await {
                Ok(ws_stream) => ws_stream,
                Err(e) => {
                    error!("WebSocket upgrade failed: {}", e);
                    break;
                }
            };

            let id = uuid::Uuid::new_v4();
            info!("New Connection: {}, {}", addr, id);

            let (write, read) = ws_stream.split();
            self.clients.lock().await.insert(
                id,
                Client {
                    write: Arc::new(Mutex::new(write)),
                    devices: HashSet::new(),
                    detection_infos: Arc::new(Mutex::new(HashMap::new())),
                    detection_value: Arc::new(Mutex::new(HashMap::new())),
                },
            );

            // 클라이언트별 메시지 처리 태스크 시작
            let clients = self.clients.clone();
            let lidars = self.lidars.clone();
            let data_sender = self.data_sender.clone();
            let data_from_udp = self.data_from_udp.clone();
            let filter_states = self.filter_states.clone();
            let filter_algorithms = self.filter_algorithms.clone();
            tokio::spawn(async move {
                let mut read_stream = read;
                loop {
                    let msg = match read_stream.next().await {
                        Some(Ok(message)) => message,
                        Some(Err(e)) => {
                            error!("WebSocket error: {}", e);
                            break;
                        }
                        None => {
                            info!("Client disconnected: {}", id);
                            break;
                        }
                    };

                    let msg_text = match msg {
                        Message::Text(text) => text,
                        Message::Binary(data) => {
                            error!("Unexpected binary data: {} bytes {:02X?}", data.len(), data);
                            continue;
                        }
                        Message::Ping(data) => {
                            // Ping에 대해 Pong으로 응답 (연결 상태 확인)
                            if let Some(client) = clients.lock().await.get_mut(&id) {
                                let mut write_lock = client.write.lock().await;
                                if let Err(e) = write_lock.send(Message::Pong(data)).await {
                                    error!("Failed to send pong: {}", e);
                                    break; // 연결에 문제가 있으면 루프 종료
                                }
                            }
                            continue;
                        }
                        Message::Pong(_) => {
                            // Pong은 단순히 무시 (연결 상태 확인 완료)
                            continue;
                        }
                        Message::Close(_) => {
                            // 클라이언트가 연결을 닫으면 루프 종료
                            info!("Client {} requested connection close", id);
                            break;
                        }
                        Message::Frame(_) => continue,
                    };

                    // 요청 ID 추출 (에러 응답용)
                    let mut req_id = "-".to_string();
                    if msg_text.len() > 0 {
                        static REQUEST_ID_RE: Lazy<Regex> =
                            Lazy::new(|| Regex::new(r#""request_id"\s*:\s*"(\d+)""#).unwrap());

                        if let Some(caps) = REQUEST_ID_RE.captures(&msg_text) {
                            if let Some(request_id) = caps.get(1) {
                                req_id = request_id.as_str().to_string();
                            }
                        }
                    }

                    let err_res = WsMessage::create_error(req_id, ErrorCode::InvalidDataFormat);

                    // JSON 메시지 파싱
                    let msg_parsed = match serde_json::from_str::<WsMessage>(&msg_text) {
                        Ok(msg) => msg,
                        Err(e) => {
                            error!("Message parse to json error: {}, {}", e, msg_text);
                            WsServer::send_message(
                                clients.clone(),
                                id,
                                serde_json::to_string(&err_res).unwrap().as_str(),
                            )
                            .await;
                            continue;
                        }
                    };

                    // 메시지 핸들러로 요청 처리
                    let res = WsHandler::request_handler(
                        msg_parsed,
                        id,
                        clients.clone(),
                        lidars.clone(),
                        data_sender.clone(),
                        data_from_udp.clone(),
                        filter_states.clone(),
                        filter_algorithms.clone(),
                    )
                    .await;

                    // 결과 처리를 즉시 수행
                    if let Ok(res) = res {
                        let message = serde_json::to_string(&res).unwrap();
                        WsServer::send_message(clients.clone(), id, &message).await;
                    } else {
                        let error_message = serde_json::to_string(&err_res).unwrap();
                        WsServer::send_message(clients.clone(), id, &error_message).await;
                    }
                }
            });
        }
    }

    /// 특정 클라이언트에게 메시지를 전송
    ///
    /// 클라이언트가 존재하는지 확인하고 WebSocket 스트림을 통해
    /// 텍스트 메시지를 전송
    ///
    /// # Arguments
    ///
    /// * `clients` - 클라이언트 맵
    /// * `id` - 대상 클라이언트의 UUID
    /// * `message` - 전송할 메시지 문자열
    pub async fn send_message(clients: Arc<Mutex<HashMap<Uuid, Client>>>, id: Uuid, message: &str) {
        let write = {
            let guard = clients.lock().await;
            if let Some(client) = guard.get(&id) {
                client.write.clone()
            } else {
                return; // 클라이언트가 존재하지 않으면 종료
            }
        }; // clients 락 해제

        let mut wl = write.lock().await;
        if let Err(e) = wl.send(Message::text(message)).await {
            error!("send message error: {}", e);
        }
    }

    /// 특정 클라이언트에게 감지 결과 Notify 메시지를 전송
    ///
    /// 감지 결과를 Notify 형태의 WebSocket 메시지로 변환하여
    /// 지정된 클라이언트에게 전송
    ///
    /// # Arguments
    ///
    /// * `clients` - 클라이언트 맵
    /// * `client_uuid` - 대상 클라이언트의 UUID
    /// * `device_id` - 장치 ID
    /// * `output_pin` - 출력 핀 상태 벡터
    /// * `area_detect` - 영역 감지 결과 벡터
    async fn send_detection_notify_message(
        clients: &Arc<Mutex<HashMap<Uuid, Client>>>,
        client_uuid: Uuid,
        device_id: &DeviceID,
        output_pin: Vec<u8>,
        area_detect: Vec<u8>,
    ) {
        let ws_message = WsMessage::Notify {
            device_id: device_id.clone(),
            data: NotifyData::DetectionResult(DetectionResult {
                output_pin,
                area_detect,
            }),
        };

        let message = serde_json::to_string(&ws_message).unwrap();

        {
            let mut lck = clients.lock().await;
            if let Some(client) = lck.get_mut(&client_uuid) {
                let mut lck = client.write.lock().await;
                let _ = lck.send(Message::text(&message)).await;
            }
        }
    }

    /// 감지 결과 처리 및 클라이언트 전송 결정
    ///
    /// 채널별 감지 결과를 DeviceDetectionState에 저장하고
    /// 요약값을 계산하여 변경사항이 있을 때만 클라이언트에게 전송
    ///
    /// # Arguments
    ///
    /// * `clients` - 클라이언트 맵
    /// * `client_uuid` - 대상 클라이언트의 UUID
    /// * `basic_info` - 장치 기본 정보
    /// * `teaching` - 티칭 모드 여부
    /// * `warning_area` - 경고 영역 설정
    /// * `device_id` - 장치 ID
    /// * `channel` - 감지된 채널 번호
    /// * `result` - 감지 결과 값
    pub async fn process_detection_result(
        clients: Arc<Mutex<HashMap<Uuid, Client>>>,
        client_uuid: Uuid,
        basic_info: BasicInfo,
        teaching: bool,
        warning_area: WarningArea,
        device_id: DeviceID,
        channel: u8,
        result: u8,
    ) {
        let (should_send, output_pin, area_detect) = {
            let guard = clients.lock().await;
            let client = match guard.get(&client_uuid) {
                Some(c) => c,
                None => return,
            };

            // 장치별 감지 상태 업데이트
            let mut det_map = client.detection_value.lock().await;
            let det = det_map
                .entry(device_id.clone())
                .or_insert_with(DeviceDetectionState::new);
            det.update_channel(channel, result);
            det.increment_sync_counter();

            // 모든 채널의 요약값 계산 (OR 연산)
            let mut summary: u8 = 0;
            for (&idx, &val) in det.channels.iter() {
                if idx == ALL_CHANNELS_SUMMARY {
                    continue; // 요약 채널은 제외
                }
                summary |= val;
            }

            // 이전 요약값과 비교
            let prev = det.channels.get(&ALL_CHANNELS_SUMMARY).copied();
            let mut output_pin: Vec<u8> = vec![
                if summary & 0x02 == 0x02 { 1 } else { 0 }, //  output 1
                if summary & 0x04 == 0x04 { 1 } else { 0 }, //  output 2
            ];
            let mut area_detect: Vec<u8> = vec![];

            // 변경 여부 판단 (요약값 변경 또는 동기화 카운터 초과)
            let mut should_send = prev.map(|p| p != summary).unwrap_or(true);
            if det.sync_counter > 100 {
                det.reset_sync_counter();
                should_send = true; // 강제 전송
            }

            // 영역 계산 (전송이 필요한 경우에만)
            if should_send {
                if !basic_info.user_area.is_empty() {
                    let pin_mode = basic_info.pulse_pin_mode.mode;
                    let pin_channel = basic_info.pulse_pin_mode.channel;

                    if pin_mode == 0 {
                        // 모드 0: 사용자 영역별 감지 결과
                        for i in 0..basic_info.user_area.len() {
                            area_detect.push(if summary & (1 << (i + 3)) > 0 { 1 } else { 0 });
                        }
                    } else if pin_mode == 1 {
                        // 모드 1: 특정 채널 제외 후 영역 계산
                        if device_id.model == 7 {
                            output_pin.push(if summary & 0x08 > 0 { 1 } else { 0 });
                            area_detect.extend(output_pin.iter().cloned());
                        } else {
                            let mut ignore_channel: Vec<u8> = vec![];
                            for i in 0..basic_info.output_channel.channel.len() {
                                if pin_channel & (1 << i) == 0 {
                                    ignore_channel.push(i as u8);
                                }
                            }
                            let mut s2 = 0;
                            for (&ch, &val) in det.channels.iter() {
                                if ch == ALL_CHANNELS_SUMMARY {
                                    continue;
                                }
                                if ignore_channel.contains(&ch) {
                                    continue; // 제외할 채널 스킵
                                }
                                s2 |= val;
                            }
                            for i in 0..basic_info.user_area.len() {
                                area_detect.push(if s2 & (1 << (i + 3)) > 0 { 1 } else { 0 });
                            }
                        }
                    } else if pin_mode == 2 {
                        // 모드 2: 장치별 특수 처리
                        if device_id.model == 3 {
                            area_detect.extend(output_pin.iter().cloned());
                        } else if device_id.model == 6 {
                            let mut s2 = 0;
                            for (&ch, &val) in det.channels.iter() {
                                if ch == ALL_CHANNELS_SUMMARY {
                                    continue;
                                }
                                let v2 = (7 | (1 << (ch + 3))) & val;
                                s2 |= v2;
                            }
                            for i in 0..basic_info.user_area.len() {
                                area_detect.push(if s2 & (1 << (i + 3)) > 0 { 1 } else { 0 });
                            }
                        }
                    }
                } else if teaching {
                    // 티칭 모드: 특정 채널 감지 결과
                    let _ = output_pin.pop();
                    area_detect.push(if summary & (1 << 3) > 0 { 1 } else { 0 });
                } else if warning_area.caution > 0.0 {
                    // 경고 영역 모드: 경고/위험 영역 감지 결과
                    output_pin.push(if summary & 0x08 > 0 { 1 } else { 0 });
                    area_detect.push(if summary & (1 << 4) > 0 { 1 } else { 0 }); // 주의
                    area_detect.push(if summary & (1 << 5) > 0 { 1 } else { 0 }); // 경고
                    area_detect.push(if summary & (1 << 6) > 0 { 1 } else { 0 }); // 위험
                }

                // 요약값 업데이트 (기존 로직 유지)
                if prev.is_some() {
                    det.update_channel(ALL_CHANNELS_SUMMARY, summary);
                } else {
                    det.update_channel(ALL_CHANNELS_SUMMARY, 0);
                }
            }

            (should_send, output_pin, area_detect)
        };

        // 변경사항이 있을 때만 클라이언트에게 전송
        if should_send {
            WsServer::send_detection_notify_message(
                &clients,
                client_uuid,
                &device_id,
                output_pin,
                area_detect,
            )
            .await;
        }
    }

    /// 스캔 결과 처리 및 클라이언트 전송
    ///
    /// UDP에서 받은 스캔 결과를 구독 중인 클라이언트들에게 전송하고
    /// 필요한 경우 감지 결과 처리도 수행
    ///
    /// # Arguments
    ///
    /// * `clients` - 클라이언트 맵
    /// * `device_id` - 장치 ID
    /// * `message` - 전송할 스캔 결과 메시지
    /// * `channel` - 스캔 채널 번호
    /// * `result` - 스캔 결과 값
    pub async fn process_scan_result(
        clients: Arc<Mutex<HashMap<Uuid, Client>>>,
        device_id: DeviceID,
        message: &str,
        channel: u8,
        result: u8,
    ) {
        // 해당 장치를 구독 중인 클라이언트들 찾기
        let clients_to_process = {
            let lck = clients.lock().await;
            lck.iter()
                .filter(|(_, client)| client.devices.contains(&device_id))
                .map(|(uuid, _)| *uuid)
                .collect::<Vec<_>>()
        };

        for client_uuid in clients_to_process {
            let clients_clone = clients.clone();
            let client = {
                let mut lck = clients_clone.lock().await;
                lck.get_mut(&client_uuid).cloned()
            };

            if let Some(client) = client {
                if client.devices.contains(&device_id) {
                    // 스캔 결과 메시지 전송
                    let ret = client.write.lock().await.send(Message::text(message)).await;
                    if let Err(e) = ret {
                        error!("send message error: {}", e);
                        // 전송 실패 시 장치 구독 해제
                        let mut lck = clients_clone.lock().await;
                        lck.get_mut(&client_uuid)
                            .unwrap()
                            .devices
                            .remove(&device_id);
                        continue;
                    }

                    // 감지 정보 초기화 (없는 경우)
                    let detection_info = {
                        let mut lck = client.detection_infos.lock().await;
                        if !lck.contains_key(&device_id) {
                            lck.insert(
                                device_id.clone(),
                                DetectionInfo {
                                    summary: 0,
                                    basic_info: BasicInfo::default(),
                                    teaching: false,
                                    warning_area: WarningArea::default(),
                                },
                            );
                        }
                        lck.get_mut(&device_id).unwrap().clone()
                    };

                    let basic_info = detection_info.basic_info;
                    let teaching = detection_info.teaching;
                    let warning_area = detection_info.warning_area;

                    // 감지 처리가 필요한지 확인
                    if basic_info.user_area.is_empty() && !teaching && warning_area.danger == 0.0 {
                        // 사용자 영역, 경고 영역, 티칭 영역 설정이 모두 없음
                        continue;
                    }

                    // 감지 결과 처리를 별도 태스크로 실행
                    tokio::spawn(async move {
                        WsServer::process_detection_result(
                            clients_clone.clone(),
                            client_uuid,
                            basic_info.clone(),
                            teaching,
                            warning_area.clone(),
                            device_id.clone(),
                            channel,
                            result,
                        )
                        .await;
                    });
                }
            }
        }
    }

    /// 장치의 감지 정보(DetectionInfo)를 업데이트
    ///
    /// 구독 중인 모든 클라이언트에서 해당 `device_id`의 정보를 찾아
    /// `summary`를 0으로 초기화한 뒤, 전달된 클로저로 특정 필드를 수정
    ///
    /// # Arguments
    ///
    /// * `clients` - 클라이언트 맵
    /// * `device_id` - 장치 ID
    /// * `f` - `DetectionInfo`를 수정하는 클로저
    pub async fn update_detection_info<F>(
        clients: &Arc<Mutex<HashMap<Uuid, Client>>>,
        device_id: &DeviceID,
        mut f: F,
    ) where
        F: FnMut(&mut DetectionInfo),
    {
        let mut clients_lck = clients.lock().await;
        for client in clients_lck.values_mut() {
            if client.devices.contains(device_id) {
                let mut infos = client.detection_infos.lock().await;
                if let Some(det) = infos.get_mut(device_id) {
                    det.summary = 0; // 요약값 초기화
                    f(det); // 클로저로 필드 수정
                }
            }
        }
    }

    /// 장치의 감지 정보를 초기화
    ///
    /// 구독 중인 모든 클라이언트에서 해당 `device_id`의 `summary`를 0으로 만들고,
    /// 채널별 감지 값 맵과 동기화 카운터를 초기화
    ///
    /// # Arguments
    ///
    /// * `clients` - 클라이언트 맵
    /// * `device_id` - 장치 ID
    pub async fn clear_detection_info(
        clients: &Arc<Mutex<HashMap<Uuid, Client>>>,
        device_id: &DeviceID,
    ) {
        let mut clients_lck = clients.lock().await;
        for client in clients_lck.values_mut() {
            if client.devices.contains(device_id) {
                {
                    let mut infos = client.detection_infos.lock().await;
                    if let Some(det) = infos.get_mut(device_id) {
                        det.summary = 0;
                    }
                }
                {
                    let mut values = client.detection_value.lock().await;
                    if let Some(map) = values.get_mut(device_id) {
                        map.channels.clear(); // 채널별 감지 값 초기화
                        map.sync_counter = 0; // 동기화 카운터 초기화
                    }
                }
            }
        }
    }

    /// UDP 모듈에서 오는 데이터를 처리하는 백그라운드 스레드를 시작
    ///
    /// UDP 수신 데이터를 파싱하여 처리
    /// - LiDAR 스캔 결과: 즉시 클라이언트에게 전달
    /// - 응답 데이터: 버퍼에 저장 (ws_handler에서 처리)
    /// - 필터링 알고리즘 적용 (활성화된 경우)
    fn data_channel_receiver(&mut self) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let clients = self.clients.clone();
        let data_receiver = self.data_receiver.take();
        let lidars = self.lidars.clone();
        let data_from_udp = self.data_from_udp.clone();
        let filter_states = self.filter_states.clone();
        let filter_algorithms = self.filter_algorithms.clone();

        std::thread::spawn(move || {
            if let Some(receiver) = data_receiver {
                while let Ok((key, data)) = receiver.recv() {
                    rt.block_on(async {
                        let device_id = utils::key_to_device_id(key);
                        let res = UdpHandler::handle_raw_data(&data);

                        match res {
                            Some(UdpHandlerResult::Notify(notify)) => match notify.clone() {
                                NotifyData::ScanResult(cloud_point) => {
                                    let mut cloud_point = cloud_point.clone();

                                    // 활성 LiDAR 장치 목록에 추가
                                    {
                                        let mut lck = lidars.lock().await;
                                        lck.insert(device_id.clone());
                                    }

                                    // 필터링 활성화 여부 확인
                                    let mut filter_enable = false;
                                    {
                                        let lck = filter_algorithms.lock().await;
                                        if lck.len() > 0 {
                                            let algorithm = lck.get(&FilterAlgorithmType::None);
                                            if let Some(_algorithm) = algorithm {
                                                // Filter Algo None이면 비활성화
                                            } else {
                                                filter_enable = true;
                                            }
                                        }
                                    }

                                    // 필터링 적용 (활성화된 경우)
                                    if filter_enable {
                                        let mut filter_states = filter_states.lock().await;

                                        match filter_states.entry(device_id.to_string()) {
                                            Entry::Occupied(mut entry) => {
                                                let filter_state = entry.get_mut();
                                                filter_state.add_point(
                                                    cloud_point.channel.to_string(),
                                                    cloud_point.clone(),
                                                );

                                                // 필터링 알고리즘 적용
                                                let history = filter_state
                                                    .get_history(&cloud_point.channel.to_string());
                                                if let Some(history) = history {
                                                    if let Some(algo_type) =
                                                        filter_algorithms.lock().await.get_mut(
                                                            &filter_state
                                                                .get_config()
                                                                .algorithm_type,
                                                        )
                                                    {
                                                        cloud_point =
                                                            algo_type.filter(&history.points);
                                                    }
                                                }
                                            }
                                            Entry::Vacant(_entry) => {
                                                // 필터 상태가 없으면 필터링 적용 안함
                                            }
                                        }
                                    }

                                    // WebSocket 메시지 생성 및 전송
                                    let ws_msg = WsMessage::Notify {
                                        device_id: device_id.clone(),
                                        data: NotifyData::ScanResult(cloud_point.clone()),
                                    };
                                    let msg: String = serde_json::to_string(&ws_msg).unwrap();

                                    WsServer::process_scan_result(
                                        clients.clone(),
                                        device_id.clone(),
                                        &msg,
                                        cloud_point.channel,
                                        data[data.len() - 2], // 결과 값은 데이터의 마지막에서 두 번째 바이트
                                    )
                                    .await;
                                }
                                _ => {}
                            },
                            Some(UdpHandlerResult::Response(response)) => {
                                // 응답 데이터에 따른 감지 정보 업데이트
                                match response.clone() {
                                    ResponseData::GetBasicInfo(basic_info) => {
                                        WsServer::update_detection_info(
                                            &clients,
                                            &device_id,
                                            |det| det.basic_info = basic_info.clone(),
                                        )
                                        .await;
                                    }
                                    ResponseData::GetTeachingArea(teaching_area) => {
                                        WsServer::update_detection_info(
                                            &clients,
                                            &device_id,
                                            |det| {
                                                det.teaching = teaching_area.area.len() > 0
                                                    && teaching_area.area[0].len() > 0
                                            },
                                        )
                                        .await;
                                    }
                                    ResponseData::GetWarningArea(warning_area) => {
                                        WsServer::update_detection_info(
                                            &clients,
                                            &device_id,
                                            |det| det.warning_area = warning_area.clone(),
                                        )
                                        .await;
                                    }
                                    ResponseData::SetPulsePinMode
                                    | ResponseData::SetOutputChannel => {
                                        // 핀 모드나 출력 채널 변경 시 감지 정보 초기화
                                        WsServer::clear_detection_info(&clients, &device_id).await;
                                    }
                                    _ => {}
                                }

                                // 응답 데이터를 버퍼에 저장
                                let mut lck = data_from_udp.lock().await;
                                lck.entry(device_id).or_insert(Vec::new()).push(response);
                            }
                            _ => {}
                        }
                    });
                }
            }
        });
    }
}
