#include <memory>
#include <cstdio>

struct FileDeleter {
    void operator()(FILE* ptr) const {
        if (ptr) {
            fclose(ptr);
        }
    }
};

class file : public SimpleListObject
{
public:
	file(Value value)
	{
		auto filename = value.as_string().data();
		file_ptr = std::unique_ptr<FILE, FileDeleter>(fopen(filename, "r"));
	}

	Value func_read() override
	{
		FILE * fp = file_ptr.get();
		fseek(fp, 0, SEEK_END); 
		auto size = ftell(fp);
		fseek(fp, 0, SEEK_SET);
		std::string fcontent(size, '\0');
		fread(fcontent.data(), 1, size, fp);
		return Value(fcontent);
	}

private:
	std::unique_ptr<FILE, FileDeleter> file_ptr;
};
