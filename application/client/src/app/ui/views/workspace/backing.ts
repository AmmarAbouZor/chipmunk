import { Session } from '@service/session';
import { IRowsPacket, Service } from '@elements/scrollarea/controllers/service';
import { Range } from '@platform/types/range';
import { Owner, RowSrc } from '@schema/content/row';
import { Nature } from '@platform/types/content';

const SCROLLAREA_SERVICE = 'workspace_scroll_area_service';

function getRows(session: Session, range: Range): Promise<IRowsPacket> {
    return new Promise((resolve, reject) => {
        session.stream
            .chunk(range)
            .then((rows) => {
                const converted: RowSrc[] = rows.map((row) => {
                    return {
                        position: row.pos,
                        content: row.content,
                        session,
                        owner: Owner.Output,
                        source:
                            typeof row.source_id === 'string'
                                ? parseInt(row.source_id, 10)
                                : row.source_id,
                        nature: new Nature(row.nature),
                    };
                });
                session.cursor.recent(converted);
                resolve({
                    rows: converted,
                    range,
                });
            })
            .catch(reject);
    });
}
export function getScrollAreaService(session: Session): Service {
    const restored = session.storage.get<Service>(SCROLLAREA_SERVICE);
    if (restored === undefined) {
        const service = new Service({
            getRows: (range: Range) => {
                return getRows(session, range);
            },
            setFrame: (range: Range) => {
                getRows(session, range)
                    .then((packet) => {
                        session.cursor.frame().set(range.asObj());
                        service.setRows(packet);
                    })
                    .catch((err: Error) => {
                        throw new Error(`Fail get  chunk: ${err.message}`);
                    });
            },
            getLen: (): number => {
                return session.stream.len();
            },
            getItemHeight: (): number => {
                return 16;
            },
        });
        service.setLen(session.stream.len());
        return service;
    } else {
        restored.setLen(session.stream.len());
        return restored;
    }
}

export function setScrollAreaService(session: Session, service: Service) {
    session.storage.set(SCROLLAREA_SERVICE, service);
}
