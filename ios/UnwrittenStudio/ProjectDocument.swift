import SwiftUI
import UniformTypeIdentifiers

extension UTType {
    static let unwrittenStudioProject = UTType(filenameExtension: "uwproj", conformingTo: .json)
        ?? UTType(exportedAs: "works.valliant.unwrittenstudio.project", conformingTo: .json)
}

struct ProjectDocument: FileDocument {
    static var readableContentTypes: [UTType] { [.unwrittenStudioProject, .json, .xml] }
    static var writableContentTypes: [UTType] { [.unwrittenStudioProject] }

    var data: Data

    init(data: Data) { self.data = data }

    init(configuration: ReadConfiguration) throws {
        guard let data = configuration.file.regularFileContents else {
            throw CocoaError(.fileReadCorruptFile)
        }
        self.data = data
    }

    func fileWrapper(configuration: WriteConfiguration) throws -> FileWrapper {
        FileWrapper(regularFileWithContents: data)
    }
}
