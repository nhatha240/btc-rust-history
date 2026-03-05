# Makefile for hft_proto — generate Python classes from .proto files
#
# Prerequisites:
#   pip install grpcio-tools
#   (or) brew install protobuf && pip install protobuf

PROTO_ROOT := ../../../proto
OUT_DIR    := .

.PHONY: gen clean

gen:
	python -m grpc_tools.protoc \
		-I$(PROTO_ROOT) \
		--python_out=./hft_proto \
		$(PROTO_ROOT)/common/envelope.proto \
		$(PROTO_ROOT)/md/features.proto \
		$(PROTO_ROOT)/ai/predictions.proto \
		$(PROTO_ROOT)/oms/orders.proto \
		$(PROTO_ROOT)/oms/execution_reports.proto \
		$(PROTO_ROOT)/oms/positions.proto
	@echo "✅ Proto generation complete"

clean:
	find . -name "*_pb2.py" -delete
	@echo "🧹 Cleaned generated files"
