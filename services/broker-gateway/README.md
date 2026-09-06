# broker-gateway
Her kurum için bir adapter (FIX 4.4 / REST / WS). Sözleşme: packages/contracts.
exec-core → exec.broker.send → adapter → kurum OMS ; adapter → exec.broker.ack|fill|reject → exec-core
TEFAS fon emirleri de aynı hattan gider (fund modülü için).
