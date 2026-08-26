import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../app.dart';

/// File sharing screen -- exposes `OnyxApi.uploadFile`/`downloadFile`
/// (backed by `mobile-core`'s `mobile_core_upload_file`/
/// `mobile_core_download_file` FFI functions on the local-first
/// transport, and an explicit "not implemented" error on the HTTP
/// transport -- see `net/onyx_http_api.dart`'s own doc comment on why:
/// `api-server` has no HTTP file route yet, confirmed by reading its
/// routes directly rather than assumed).
///
/// Takes a filesystem path rather than integrating a file-picker
/// package: no file-picker dependency exists in `pubspec.yaml` today,
/// and this sandbox has no Flutter/Dart toolchain to verify a new
/// native dependency actually builds on-device -- adding one
/// unverified would be a bigger, riskier change than this screen
/// itself. A real "pick a file" UI (`file_picker` or similar) is a
/// natural follow-up once that can be built and tested on a real
/// device/CI, flagged here rather than done speculatively.
class FilesScreen extends StatefulWidget {
  const FilesScreen({super.key});

  @override
  State<FilesScreen> createState() => _FilesScreenState();
}

class _FilesScreenState extends State<FilesScreen> {
  final _uploadPathController = TextEditingController();
  final _downloadHashController = TextEditingController();
  final _downloadDestController = TextEditingController();

  bool _busy = false;
  String? _status;
  Map<String, dynamic>? _lastUpload;

  @override
  void dispose() {
    _uploadPathController.dispose();
    _downloadHashController.dispose();
    _downloadDestController.dispose();
    super.dispose();
  }

  Future<void> _upload(OnyxController controller) async {
    final path = _uploadPathController.text.trim();
    if (path.isEmpty) return;
    setState(() {
      _busy = true;
      _status = null;
    });
    try {
      final outcome = await controller.api.uploadFile(path);
      setState(() {
        _lastUpload = outcome;
        _status = 'Uploaded ${outcome['size_bytes']} bytes. Content hash: ${outcome['content_hash']}';
      });
    } catch (error) {
      setState(() => _status = 'Upload failed: $error');
    } finally {
      setState(() => _busy = false);
    }
  }

  Future<void> _download(OnyxController controller) async {
    final hash = _downloadHashController.text.trim();
    final dest = _downloadDestController.text.trim();
    if (hash.isEmpty || dest.isEmpty) return;
    setState(() {
      _busy = true;
      _status = null;
    });
    try {
      final bytesWritten = await controller.api.downloadFile(hash, dest);
      setState(() => _status = 'Downloaded $bytesWritten bytes to $dest');
    } catch (error) {
      setState(() => _status = 'Download failed: $error');
    } finally {
      setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final controller = context.watch<OnyxController>();
    return ListView(
      padding: const EdgeInsets.all(16),
      children: <Widget>[
        Text('Files', style: Theme.of(context).textTheme.headlineSmall),
        const SizedBox(height: 16),
        Card(
          child: Padding(
            padding: const EdgeInsets.all(16),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                const Text('Upload a file', style: TextStyle(fontWeight: FontWeight.bold)),
                const SizedBox(height: 8),
                TextField(
                  controller: _uploadPathController,
                  decoration: const InputDecoration(labelText: 'File path on this device'),
                ),
                const SizedBox(height: 8),
                ElevatedButton(
                  onPressed: _busy ? null : () => _upload(controller),
                  child: const Text('Upload'),
                ),
                if (_lastUpload != null) ...<Widget>[
                  const SizedBox(height: 8),
                  SelectableText('content_hash: ${_lastUpload!['content_hash']}'),
                ],
              ],
            ),
          ),
        ),
        const SizedBox(height: 16),
        Card(
          child: Padding(
            padding: const EdgeInsets.all(16),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                const Text('Download a file', style: TextStyle(fontWeight: FontWeight.bold)),
                const SizedBox(height: 8),
                TextField(
                  controller: _downloadHashController,
                  decoration: const InputDecoration(labelText: 'Content hash'),
                ),
                const SizedBox(height: 8),
                TextField(
                  controller: _downloadDestController,
                  decoration: const InputDecoration(labelText: 'Save to path on this device'),
                ),
                const SizedBox(height: 8),
                ElevatedButton(
                  onPressed: _busy ? null : () => _download(controller),
                  child: const Text('Download'),
                ),
              ],
            ),
          ),
        ),
        if (_status != null) ...<Widget>[
          const SizedBox(height: 16),
          Text(_status!),
        ],
      ],
    );
  }
}
