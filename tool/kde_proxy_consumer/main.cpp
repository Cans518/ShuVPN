#include <KIO/StoredTransferJob>
#include <KJob>
#include <QCoreApplication>
#include <QTimer>
#include <QUrl>
#include <cstdio>

int main(int argc, char **argv) {
    QCoreApplication app(argc, argv);
    if (argc != 2) {
        return 2;
    }
    auto *job = KIO::storedGet(QUrl(QString::fromUtf8(argv[1])), KIO::NoReload, KIO::HideProgressInfo);
    job->setUiDelegate(nullptr);
    QObject::connect(job, &KJob::result, &app, [job, &app]() {
        if (job->error() || job->data() != QByteArray("kde-proxy-response")) {
            std::fprintf(stderr, "KIO proxy response failed: %s\n",
                         job->errorString().toUtf8().constData());
            app.exit(1);
            return;
        }
        std::puts("KIO_SYSTEM_PROXY_HTTP_BYTES_VERIFIED");
        app.exit(0);
    });
    QTimer::singleShot(10000, &app, [&app]() {
        std::fputs("KIO proxy consumer timed out\n", stderr);
        app.exit(1);
    });
    return app.exec();
}
